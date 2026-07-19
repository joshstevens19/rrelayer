use std::{sync::Arc, time::Duration};

use alloy::{
    dyn_abi::{DynSolType, JsonAbiExt},
    json_abi::Function,
    primitives::Bytes,
};
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use tokio::time;
use tracing::{error, info};

use crate::{
    postgres::{PostgresClient, PostgresError},
    relayer::Relayer,
    shutdown::subscribe_to_shutdown,
    transaction::{
        queue_system::{TransactionToSend, TransactionsQueues},
        types::{TransactionData, TransactionValue},
    },
    yaml::{
        parse_cron_job_interval, AllOrOneOrManyAddresses, CronJobConfig, CronJobTransactionConfig,
        SetupConfig,
    },
};

impl PostgresClient {
    async fn get_cron_job_last_ran_at(
        &self,
        project_name: &str,
        job_name: &str,
    ) -> Result<Option<DateTime<Utc>>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT last_ran_at
                    FROM relayer.cron_job_state
                    WHERE project_name = $1
                    AND job_name = $2;
                ",
                &[&project_name, &job_name],
            )
            .await?;

        Ok(row.map(|row| row.get("last_ran_at")))
    }

    async fn mark_cron_job_ran(
        &self,
        project_name: &str,
        job_name: &str,
        ran_at: DateTime<Utc>,
    ) -> Result<(), PostgresError> {
        self.execute(
            "
                INSERT INTO relayer.cron_job_state(project_name, job_name, last_ran_at, updated_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (project_name, job_name)
                DO UPDATE SET last_ran_at = EXCLUDED.last_ran_at, updated_at = NOW();
            ",
            &[&project_name, &job_name, &ran_at],
        )
        .await?;

        Ok(())
    }
}

pub fn run_cron_job_tasks(
    config: SetupConfig,
    postgres_client: Arc<PostgresClient>,
    transactions_queues: Arc<tokio::sync::Mutex<TransactionsQueues>>,
) {
    let cron_jobs = match config.cron_jobs.clone() {
        Some(cron_jobs) => cron_jobs,
        None => {
            info!("Cron jobs disabled - no cron_jobs configuration found");
            return;
        }
    };

    let enabled_jobs: Vec<CronJobConfig> =
        cron_jobs.into_iter().filter(|cron_job| cron_job.enabled).collect();

    if enabled_jobs.is_empty() {
        info!("Cron jobs disabled - no enabled cron_jobs found");
        return;
    }

    info!("Starting {} cron job background task(s)", enabled_jobs.len());

    for cron_job in enabled_jobs {
        let postgres_client = postgres_client.clone();
        let transactions_queues = transactions_queues.clone();
        let config = config.clone();

        tokio::spawn(async move {
            run_cron_job(config, cron_job, postgres_client, transactions_queues).await;
        });
    }
}

async fn run_cron_job(
    config: SetupConfig,
    cron_job: CronJobConfig,
    postgres_client: Arc<PostgresClient>,
    transactions_queues: Arc<tokio::sync::Mutex<TransactionsQueues>>,
) {
    let interval_duration = match parse_cron_job_interval(&cron_job.schedule.every) {
        Ok(duration) => duration,
        Err(err) => {
            error!("Cron job {} has invalid interval: {}", cron_job.name, err);
            return;
        }
    };

    let mut shutdown_rx = subscribe_to_shutdown();

    let startup_delay =
        next_cron_job_delay(&config, &cron_job, &postgres_client, interval_duration).await;

    if wait_for_delay_or_shutdown(startup_delay, &mut shutdown_rx, &cron_job.name).await {
        return;
    }

    loop {
        enqueue_cron_job_transaction(&config, &cron_job, &postgres_client, &transactions_queues)
            .await;

        if wait_for_delay_or_shutdown(interval_duration, &mut shutdown_rx, &cron_job.name).await {
            break;
        }
    }
}

async fn next_cron_job_delay(
    config: &SetupConfig,
    cron_job: &CronJobConfig,
    postgres_client: &PostgresClient,
    interval_duration: Duration,
) -> Duration {
    match postgres_client.get_cron_job_last_ran_at(&config.name, &cron_job.name).await {
        Ok(Some(last_ran_at)) => {
            let next_run_at = last_ran_at + interval_duration;
            let now = Utc::now();

            if next_run_at <= now {
                Duration::ZERO
            } else {
                (next_run_at - now).to_std().unwrap_or(Duration::ZERO)
            }
        }
        Ok(None) => {
            if cron_job.schedule.run_on_startup.unwrap_or(false) {
                Duration::ZERO
            } else {
                // Anchor the schedule now, otherwise a restart before the first run
                // would reset the initial interval and the job could never fire.
                if let Err(err) = postgres_client
                    .mark_cron_job_ran(&config.name, &cron_job.name, Utc::now())
                    .await
                {
                    error!(
                        "Cron job {} failed to persist schedule anchor: {}",
                        cron_job.name, err
                    );
                }

                interval_duration
            }
        }
        Err(err) => {
            error!(
                "Cron job {} failed to read last run state, waiting full interval: {}",
                cron_job.name, err
            );
            interval_duration
        }
    }
}

async fn wait_for_delay_or_shutdown(
    delay: Duration,
    shutdown_rx: &mut tokio::sync::broadcast::Receiver<()>,
    cron_job_name: &str,
) -> bool {
    if delay.is_zero() {
        return false;
    }

    tokio::select! {
        _ = time::sleep(delay) => false,
        _ = shutdown_rx.recv() => {
            info!("Shutdown signal received, stopping cron job {}", cron_job_name);
            true
        }
    }
}

async fn enqueue_cron_job_transaction(
    config: &SetupConfig,
    cron_job: &CronJobConfig,
    postgres_client: &PostgresClient,
    transactions_queues: &Arc<tokio::sync::Mutex<TransactionsQueues>>,
) {
    let relayer = match select_cron_job_relayer(config, cron_job, postgres_client).await {
        Ok(relayer) => relayer,
        Err(err) => {
            error!(
                "Cron job {} could not select relayer on network {}: {}",
                cron_job.name, cron_job.network, err
            );
            return;
        }
    };

    let transaction_to_send = match build_transaction_to_send(&cron_job.transaction) {
        Ok(transaction_to_send) => transaction_to_send,
        Err(err) => {
            error!("Cron job {} has invalid transaction: {}", cron_job.name, err);
            return;
        }
    };

    match transactions_queues.lock().await.add_transaction(&relayer.id, &transaction_to_send).await
    {
        Ok(transaction) => {
            if let Err(err) =
                postgres_client.mark_cron_job_ran(&config.name, &cron_job.name, Utc::now()).await
            {
                error!("Cron job {} failed to persist last run state: {}", cron_job.name, err);
            }

            info!(
                "Cron job {} queued transaction {} for relayer {}",
                cron_job.name, transaction.id, relayer.id
            );
        }
        Err(err) => {
            error!(
                "Cron job {} failed to queue transaction for relayer {}: {}",
                cron_job.name, relayer.id, err
            );
        }
    }
}

async fn select_cron_job_relayer(
    config: &SetupConfig,
    cron_job: &CronJobConfig,
    postgres_client: &PostgresClient,
) -> Result<Relayer, String> {
    let network_config = config
        .networks
        .iter()
        .find(|network| network.name == cron_job.network)
        .ok_or_else(|| format!("network {} is not configured", cron_job.network))?;

    let relayers = postgres_client
        .get_all_relayers_for_chain(&network_config.chain_id)
        .await
        .map_err(|err| err.to_string())?;

    let mut available_relayers: Vec<_> = relayers
        .into_iter()
        .filter(|relayer| !relayer.paused)
        .filter(|relayer| match &cron_job.relayers {
            AllOrOneOrManyAddresses::All => true,
            relayers => relayers.contains(&relayer.address),
        })
        .filter(|relayer| {
            validate_cron_job_transaction_permissions(config, cron_job, relayer).is_ok()
        })
        .collect();

    let mut rng = rand::thread_rng();
    available_relayers.shuffle(&mut rng);

    available_relayers.into_iter().next().ok_or_else(|| {
        format!(
            "no available relayers found for network {} matching cron job relayers filter and permissions",
            cron_job.network
        )
    })
}

fn validate_cron_job_transaction_permissions(
    config: &SetupConfig,
    cron_job: &CronJobConfig,
    relayer: &Relayer,
) -> Result<(), String> {
    let network_config =
        match config.networks.iter().find(|network| network.chain_id == relayer.chain_id) {
            Some(network_config) => network_config,
            None => return Ok(()),
        };

    let Some(permissions) = &network_config.permissions else {
        return Ok(());
    };

    for permission in permissions {
        if !permission.relayers.contains(&relayer.address) {
            continue;
        }

        if permission.disable_transactions.unwrap_or_default() {
            return Err("relayer has transactions disabled".to_string());
        }

        if permission.disable_native_transfer.unwrap_or_default()
            && cron_job
                .transaction
                .value
                .as_deref()
                .unwrap_or("0")
                .parse::<TransactionValue>()
                .map(|value| !value.is_zero())
                .unwrap_or(false)
        {
            return Err("relayer has native transfers disabled".to_string());
        }

        if !permission.allowlist.is_empty()
            && !permission.allowlist.contains(&cron_job.transaction.to)
        {
            return Err(format!(
                "relayer is not allowed to send transactions to {}",
                cron_job.transaction.to
            ));
        }
    }

    Ok(())
}

fn build_transaction_to_send(
    transaction: &CronJobTransactionConfig,
) -> Result<TransactionToSend, String> {
    let value = transaction
        .value
        .as_deref()
        .unwrap_or("0")
        .parse::<TransactionValue>()
        .map_err(|err| format!("invalid value: {err}"))?;

    let data = match (&transaction.data, &transaction.contract) {
        (Some(data), None) => TransactionData::raw_hex(data)?,
        (None, Some(contract)) => encode_contract_call(&contract.function, &contract.args)?,
        (None, None) => {
            return Err("transaction.data or transaction.contract is required".to_string())
        }
        (Some(_), Some(_)) => {
            return Err(
                "transaction.data and transaction.contract cannot both be defined".to_string()
            );
        }
    };

    Ok(TransactionToSend::new(
        transaction.to,
        value,
        data,
        transaction.speed.clone(),
        None,
        transaction.external_id.clone(),
    ))
}

fn encode_contract_call(function: &str, args: &[String]) -> Result<TransactionData, String> {
    let function = Function::parse(function)
        .map_err(|err| format!("invalid contract function signature: {err}"))?;

    if function.inputs.len() != args.len() {
        return Err(format!(
            "function expects {} arg(s), got {}",
            function.inputs.len(),
            args.len()
        ));
    }

    let values = function
        .inputs
        .iter()
        .zip(args)
        .map(|(param, arg)| {
            let ty = param
                .ty
                .parse::<DynSolType>()
                .map_err(|err| format!("invalid ABI parameter type {}: {err}", param.ty))?;

            ty.coerce_str(arg)
                .map_err(|err| format!("invalid ABI arg {} for type {}: {err}", arg, param.ty))
        })
        .collect::<Result<Vec<_>, _>>()?;

    function
        .abi_encode_input(&values)
        .map(|encoded| TransactionData::new(Bytes::from(encoded)))
        .map_err(|err| format!("failed to ABI encode contract call: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_transaction_from_raw_data() {
        let transaction = CronJobTransactionConfig {
            to: "0x0000000000000000000000000000000000000001".parse().unwrap(),
            value: Some("1".to_string()),
            speed: None,
            external_id: Some("job-1".to_string()),
            data: Some("0x1234".to_string()),
            contract: None,
        };

        let transaction_to_send = build_transaction_to_send(&transaction).unwrap();
        assert_eq!(transaction_to_send.to, transaction.to);
        assert_eq!(transaction_to_send.value, "1".parse::<TransactionValue>().unwrap());
        assert_eq!(transaction_to_send.data.hex(), "1234");
        assert_eq!(transaction_to_send.external_id, Some("job-1".to_string()));
    }

    #[test]
    fn encodes_contract_call_data() {
        let data = encode_contract_call(
            "transfer(address,uint256)",
            &["0x0000000000000000000000000000000000000001".to_string(), "100".to_string()],
        )
        .unwrap();

        assert_eq!(
            data.hex(),
            "a9059cbb00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000064"
        );
    }
}
