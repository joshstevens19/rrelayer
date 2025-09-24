use std::{collections::VecDeque, sync::Arc};

use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::{transactions_queues::TransactionsQueues, types::TransactionRelayerSetup};
use crate::transaction::queue_system::types::{
    ProcessInmempoolTransactionError, ProcessMinedTransactionError, ProcessPendingTransactionError,
};
use crate::webhooks::WebhookManager;
use crate::{
    gas::{BlobGasOracleCache, GasOracleCache},
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{find_provider_for_chain_id, EvmProvider},
    relayer::{Relayer, RelayerId},
    rrelayer_error,
    safe_proxy::SafeProxyManager,
    shared::{
        cache::Cache,
        common_types::{PagingContext, WalletOrProviderError},
        utils::sleep_ms,
    },
    transaction::types::{Transaction, TransactionStatus},
};

/// Spawns processing tasks for a single relayer.
pub async fn spawn_processing_tasks_for_relayer(
    transaction_queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: &RelayerId,
) {
    let queue_clone_pending = transaction_queue.clone();
    let relayer_id_pending = *relayer_id;
    tokio::spawn(async move {
        continuously_process_pending_transactions(queue_clone_pending, &relayer_id_pending).await;
    });

    let queue_clone_inmempool = transaction_queue.clone();
    let relayer_id_inmempool = *relayer_id;
    tokio::spawn(async move {
        continuously_process_inmempool_transactions(queue_clone_inmempool, &relayer_id_inmempool)
            .await;
    });

    let queue_clone_mined = transaction_queue.clone();
    let relayer_id_mined = *relayer_id;
    tokio::spawn(async move {
        continuously_process_mined_transactions(queue_clone_mined, &relayer_id_mined).await;
    });
}

/// Spawns background processing tasks for all transaction queues.
async fn spawn_processing_tasks(transaction_queue: Arc<Mutex<TransactionsQueues>>) {
    let relay_ids: Vec<RelayerId> =
        { transaction_queue.lock().await.queues.keys().cloned().collect() };

    for relayer_id in relay_ids {
        spawn_processing_tasks_for_relayer(transaction_queue.clone(), &relayer_id).await;
    }
}

/// Pauses processing for the specified duration.
async fn processes_next_break(process_again_after_ms: &u64) {
    sleep_ms(process_again_after_ms).await
}

/// Continuously processes pending transactions for a specific relayer.
///
/// Runs in an infinite loop, processing one pending transaction at a time
/// and waiting for the specified delay between iterations.
async fn continuously_process_pending_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: &RelayerId,
) {
    loop {
        let result = {
            let mut tq = queue.lock().await;
            tq.process_single_pending(relayer_id).await
        };

        match result {
            Ok(result) => {
                // info!("PENDING: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => {
                match e {
                    ProcessPendingTransactionError::RelayerTransactionsQueueNotFound(_) => {
                        // the queue has been deleted so kill out the loop
                        info!(
                            "Relayer id {} has been deleted stopping the pending queue for it",
                            relayer_id
                        );
                        break;
                    }
                    _ => {
                        error!("PENDING ERROR: {}", e)
                    }
                }
            }
        }
    }
}

/// Continuously processes in-mempool transactions for a specific relayer.
///
/// Runs in an infinite loop, processing one in-mempool transaction at a time
/// and waiting for the specified delay between iterations.
async fn continuously_process_inmempool_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: &RelayerId,
) {
    loop {
        let result = {
            let mut tq = queue.lock().await;
            tq.process_single_inmempool(relayer_id).await
        };

        match result {
            Ok(result) => {
                // info!("INMEMPOOL: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => {
                match e {
                    ProcessInmempoolTransactionError::RelayerTransactionsQueueNotFound(_) => {
                        // the queue has been deleted so kill out the loop
                        info!(
                            "Relayer id {} has been deleted stopping the inmempool queue for it",
                            relayer_id
                        );
                        break;
                    }
                    _ => {
                        error!("INMEMPOOL ERROR: {}", e)
                    }
                }
            }
        }
    }
}

/// Continuously processes mined transactions for a specific relayer.
///
/// Runs in an infinite loop, processing one mined transaction at a time
/// to check for confirmations and waiting for the specified delay between iterations.
async fn continuously_process_mined_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: &RelayerId,
) {
    loop {
        let result = {
            let mut tq = queue.lock().await;
            tq.process_single_mined(relayer_id).await
        };

        match result {
            Ok(result) => {
                // info!("MINED: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => {
                match e {
                    ProcessMinedTransactionError::RelayerTransactionsQueueNotFound(_) => {
                        // the queue has been deleted so kill out the loop
                        info!(
                            "Relayer id {} has been deleted stopping the mined queue for it",
                            relayer_id
                        );
                        break;
                    }
                    _ => {
                        error!("MINED ERROR: {}", e)
                    }
                }
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum RepopulateTransactionsQueueError {
    #[error("Failed to load transactions with status {0} for relayer {1} from database: {1}")]
    CouldNotGetTransactionsByStatusFromDatabase(TransactionStatus, RelayerId, PostgresError),
}

/// Repopulates a transaction queue from the database for a specific status.
///
/// Loads all transactions with the given status for a relayer from the database,
/// maintaining their nonce order in the queue.
async fn repopulate_transaction_queue(
    db: &PostgresClient,
    relayer_id: &RelayerId,
    status: &TransactionStatus,
) -> Result<VecDeque<Transaction>, RepopulateTransactionsQueueError> {
    let mut transactions_queue: VecDeque<Transaction> = VecDeque::new();
    let mut paging_context = PagingContext::new(1000, 0);
    loop {
        let results = db
            .get_transactions_by_status_for_relayer(relayer_id, status, &paging_context)
            .await
            .map_err(|e| {
                RepopulateTransactionsQueueError::CouldNotGetTransactionsByStatusFromDatabase(
                    status.clone(),
                    *relayer_id,
                    e,
                )
            })?;

        let result_count = results.items.len();

        for item in results.items {
            // as this will come back as 0,1,2,3,4 we push back each time as ordered by nonce
            transactions_queue.push_back(item)
        }

        let next = paging_context.next(result_count);
        match next {
            Some(next) => paging_context = next,
            None => break,
        }
    }

    Ok(transactions_queue)
}

/// Loads all relayers from the database.
async fn load_relayers(db: &PostgresClient) -> Result<Vec<Relayer>, PostgresError> {
    let mut relayers: Vec<Relayer> = Vec::new();
    let mut paging_context = PagingContext::new(1000, 0);
    loop {
        let results = db.get_relayers(&paging_context).await?;

        let result_count = results.items.len();

        for item in results.items {
            relayers.push(item)
        }

        let next = paging_context.next(result_count);
        match next {
            Some(next) => paging_context = next,
            None => break,
        }
    }

    Ok(relayers)
}

#[derive(Error, Debug)]
pub enum StartTransactionsQueuesError {
    #[error("Failed to connect to the database: {0}")]
    DatabaseConnectionError(PostgresConnectionError),

    #[error("Failed to load relayers from database: {0}")]
    CouldNotLoadRelayersFromDatabase(PostgresError),

    #[error("Failed to repopulate transactions queue: {0}")]
    RepopulateTransactionsQueueError(#[from] RepopulateTransactionsQueueError),

    #[error("Failed to init transactions queues: {0}")]
    CouldNotInitTransactionsQueues(#[from] WalletOrProviderError),

    #[error("Transactions queues error: {0}")]
    TransactionsQueuesError(
        #[from] crate::transaction::queue_system::transactions_queues::TransactionsQueuesError,
    ),
}

pub async fn startup_transactions_queues(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    safe_proxy_manager: Arc<SafeProxyManager>,
) -> Result<Arc<Mutex<TransactionsQueues>>, StartTransactionsQueuesError> {
    let postgres = PostgresClient::new()
        .await
        .map_err(StartTransactionsQueuesError::DatabaseConnectionError)?;

    // has to load them ALL to populate their queues
    let relays = load_relayers(&postgres)
        .await
        .map_err(StartTransactionsQueuesError::CouldNotLoadRelayersFromDatabase)?;

    let mut transaction_relayer_setups: Vec<TransactionRelayerSetup> = Vec::new();

    for relayer in relays {
        let provider = find_provider_for_chain_id(&providers, &relayer.chain_id).await;

        match provider {
            None => {
                warn!("Could not find network provider on chain {} this means relayer name {} - id {} has not been started up make sure the network is enabled in your yaml.. skipping", relayer.chain_id, relayer.name, relayer.id);
                continue;
            }
            Some(provider) => {
                let evm_provider = provider.clone();

                let relayer_id = relayer.id;

                let mined_transactions =
                    repopulate_transaction_queue(&postgres, &relayer_id, &TransactionStatus::Mined)
                        .await?;

                transaction_relayer_setups.push(TransactionRelayerSetup::new(
                    relayer,
                    evm_provider,
                    repopulate_transaction_queue(
                        &postgres,
                        &relayer_id,
                        &TransactionStatus::Pending,
                    )
                    .await?,
                    repopulate_transaction_queue(
                        &postgres,
                        &relayer_id,
                        &TransactionStatus::Inmempool,
                    )
                    .await?,
                    mined_transactions
                        .into_iter()
                        .map(|transaction| (transaction.id, transaction))
                        .collect(),
                ));
            }
        }
    }

    let transactions_queues = Arc::new(Mutex::new(
        TransactionsQueues::new(
            transaction_relayer_setups,
            gas_oracle_cache,
            blob_gas_oracle_cache,
            cache,
            webhook_manager,
            safe_proxy_manager,
        )
        .await?,
    ));

    spawn_processing_tasks(transactions_queues.clone()).await;

    Ok(transactions_queues)
}
