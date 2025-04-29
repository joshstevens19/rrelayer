use std::{collections::VecDeque, sync::Arc};

use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info};

use super::{transactions_queues::TransactionsQueues, types::TransactionRelayerSetup};
use crate::{
    gas::gas_oracle::GasOracleCache,
    network::types::ChainId,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{find_provider_for_chain_id, EvmProvider},
    relayer::types::{Relayer, RelayerId},
    shared::{
        cache::Cache,
        common_types::{PagingContext, WalletOrProviderError},
        utils::sleep_ms,
    },
    transaction::types::{Transaction, TransactionStatus},
};

async fn spawn_processing_tasks(transaction_queue: Arc<Mutex<TransactionsQueues>>) {
    let relay_ids: Vec<RelayerId> =
        { transaction_queue.lock().await.queues.lock().await.keys().cloned().collect() };

    for relayer_id in relay_ids {
        let queue_clone_pending = transaction_queue.clone();
        tokio::spawn(async move {
            continuously_process_pending_transactions(queue_clone_pending, relayer_id).await;
        });

        let queue_clone_inmempool = transaction_queue.clone();
        tokio::spawn(async move {
            continuously_process_inmempool_transactions(queue_clone_inmempool, relayer_id).await;
        });

        let queue_clone_mined = transaction_queue.clone();
        tokio::spawn(async move {
            continuously_process_mined_transactions(queue_clone_mined, relayer_id).await;
        });
    }
}

async fn processes_next_break(process_again_after_ms: &u64) {
    sleep_ms(process_again_after_ms).await
}

async fn continuously_process_pending_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: RelayerId,
) {
    loop {
        let result = {
            let mut tq = queue.lock().await;
            tq.process_single_pending(&relayer_id).await
        };

        match result {
            Ok(result) => {
                info!("PENDING: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => error!("PENDING ERROR: {}", e),
        }
    }
}

async fn continuously_process_inmempool_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: RelayerId,
) {
    loop {
        let result = {
            let mut tq = queue.lock().await;
            tq.process_single_inmempool(&relayer_id).await
        };

        match result {
            Ok(result) => {
                info!("INMEMPOOL: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => error!("INMEMPOOL ERROR: {}", e),
        }
    }
}

async fn continuously_process_mined_transactions(
    queue: Arc<Mutex<TransactionsQueues>>,
    relayer_id: RelayerId,
) {
    loop {
        let result = {
            // Lock the mutex to get a reference to the TransactionQueue
            let mut tq = queue.lock().await;
            // Call process_single_mined on the TransactionQueue reference
            tq.process_single_mined(&relayer_id).await
        };

        match result {
            Ok(result) => {
                info!("MINED: {:?}", result);
                processes_next_break(&result.process_again_after).await;
            }
            Err(e) => error!("MINED ERROR: {}", e),
        }
    }
}

#[derive(Error, Debug)]
pub enum RepopulateTransactionsQueueError {
    #[error("Failed to load transactions with status {0} for relayer {1} from database: {1}")]
    CouldNotGetTransactionsByStatusFromDatabase(TransactionStatus, RelayerId, PostgresError),
}

async fn repopulate_transaction_queue(
    db: &PostgresClient,
    relayer_id: &RelayerId,
    status: &TransactionStatus,
) -> Result<VecDeque<Transaction>, RepopulateTransactionsQueueError> {
    // now load any state transactions which need to be reloaded in the queues
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

    #[error("Could not find provider for chai {0}")]
    CouldNotFindProviderForChainId(ChainId),

    #[error("Failed to repopulate transactions queue: {0}")]
    RepopulateTransactionsQueueError(RepopulateTransactionsQueueError),

    #[error("Failed to init transactions queues: {0}")]
    CouldNotInitTransactionsQueues(#[from] WalletOrProviderError),
}

pub async fn startup_transactions_queues(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
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
                Err(StartTransactionsQueuesError::CouldNotFindProviderForChainId(relayer.chain_id))?
            }
            Some(provider) => {
                let evm_provider = provider.clone();

                let relayer_id = relayer.id;

                let mined_transactions =
                    repopulate_transaction_queue(&postgres, &relayer_id, &TransactionStatus::Mined)
                        .await
                        .map_err(StartTransactionsQueuesError::RepopulateTransactionsQueueError)?;

                transaction_relayer_setups.push(TransactionRelayerSetup::new(
                    relayer,
                    evm_provider,
                    repopulate_transaction_queue(
                        &postgres,
                        &relayer_id,
                        &TransactionStatus::Pending,
                    )
                    .await
                    .map_err(StartTransactionsQueuesError::RepopulateTransactionsQueueError)?,
                    repopulate_transaction_queue(
                        &postgres,
                        &relayer_id,
                        &TransactionStatus::Inmempool,
                    )
                    .await
                    .map_err(StartTransactionsQueuesError::RepopulateTransactionsQueueError)?,
                    mined_transactions
                        .into_iter()
                        .map(|transaction| (transaction.id, transaction))
                        .collect(),
                ));
            }
        }
    }

    let transactions_queues = Arc::new(Mutex::new(
        TransactionsQueues::new(transaction_relayer_setups, gas_oracle_cache, cache).await?,
    ));

    spawn_processing_tasks(transactions_queues.clone()).await;

    Ok(transactions_queues)
}
