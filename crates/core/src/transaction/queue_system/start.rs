use std::{collections::VecDeque, sync::Arc};

use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::{transactions_queues::TransactionsQueues, types::TransactionRelayerSetup};
use crate::transaction::queue_system::types::{
    CompetitionType, CompetitiveTransaction, ProcessInmempoolTransactionError,
    ProcessMinedTransactionError, ProcessPendingTransactionError,
};
use crate::webhooks::WebhookManager;
use crate::{
    gas::{BlobGasOracleCache, GasOracleCache},
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{find_provider_for_chain_id, EvmProvider},
    relayer::{Relayer, RelayerId},
    safe_proxy::SafeProxyManager,
    shared::{
        cache::Cache,
        common_types::{PagingContext, WalletOrProviderError},
        utils::sleep_ms,
    },
    shutdown::subscribe_to_shutdown,
    transaction::types::{Transaction, TransactionStatus},
};

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
    let mut shutdown_rx = subscribe_to_shutdown();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping pending queue for relayer {}", relayer_id);
                break;
            }
            result = async {
                let mut tq = queue.lock().await;
                tq.process_single_pending(relayer_id).await
            } => {
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
                                error!("Relayer id {} - PENDING QUEUE ERROR: {}", relayer_id, e)
                            }
                        }
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
    let mut shutdown_rx = subscribe_to_shutdown();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping inmempool queue for relayer {}", relayer_id);
                break;
            }
            result = async {
                let mut tq = queue.lock().await;
                tq.process_single_inmempool(relayer_id).await
            } => {
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
                                error!("Relayer id {} - INMEMPOOL QUEUE ERROR: {}", relayer_id, e)
                            }
                        }
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
    let mut shutdown_rx = subscribe_to_shutdown();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping mined queue for relayer {}", relayer_id);
                break;
            }
            result = async {
                let mut tq = queue.lock().await;
                tq.process_single_mined(relayer_id).await
            } => {
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
                                error!("Relayer id {} - MINED QUEUE ERROR: {}", relayer_id, e)
                            }
                        }
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
                    *status,
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

/// Reconstructs competitive transactions from the database for inmempool status.
///
/// This function handles the complex logic of rebuilding the competitive transaction
/// structure from individual database records by:
/// 1. Loading all INMEMPOOL transactions
/// 2. Identifying competitions via cancelled_by_transaction_id linkage
/// 3. Determining competition type based on transaction content
/// 4. Building proper CompetitiveTransaction objects
async fn repopulate_competitive_transaction_queue(
    db: &PostgresClient,
    relayer_id: &RelayerId,
) -> Result<VecDeque<CompetitiveTransaction>, RepopulateTransactionsQueueError> {
    // Load all INMEMPOOL transactions
    let inmempool_transactions =
        repopulate_transaction_queue(db, relayer_id, &TransactionStatus::INMEMPOOL).await?;

    let mut competitive_queue: VecDeque<CompetitiveTransaction> = VecDeque::new();
    let mut processed_transaction_ids = std::collections::HashSet::new();

    for transaction in &inmempool_transactions {
        // Skip if we already processed this transaction as part of a competition
        if processed_transaction_ids.contains(&transaction.id) {
            continue;
        }

        // Check if this transaction has a competitor
        if let Some(cancel_tx_id) = &transaction.cancelled_by_transaction_id {
            // This is an original transaction with a competitor
            // Find the competitor transaction
            if let Some(competitor) =
                inmempool_transactions.iter().find(|tx| tx.id == *cancel_tx_id)
            {
                // Determine competition type based on transaction content
                let is_empty_data = competitor.data.clone().into_inner().is_empty();
                let competition_type =
                    if competitor.is_noop || (competitor.value.is_zero() && is_empty_data) {
                        CompetitionType::Cancel
                    } else {
                        CompetitionType::Replace
                    };

                // Create competitive transaction with original and competitor
                let mut comp_tx = CompetitiveTransaction::new(transaction.clone());
                comp_tx.add_competitor(competitor.clone(), competition_type.clone());

                competitive_queue.push_back(comp_tx);

                // Mark both transactions as processed
                processed_transaction_ids.insert(transaction.id);
                processed_transaction_ids.insert(competitor.id);

                info!("Reconstructed competitive transaction: original {} with competitor {} (type: {:?})",
                    transaction.id, competitor.id, competition_type);
            } else {
                warn!("Transaction {} has cancelled_by_transaction_id {} but competitor not found in INMEMPOOL",
                    transaction.id, cancel_tx_id);

                // Add as single transaction since competitor not found
                competitive_queue.push_back(CompetitiveTransaction::new(transaction.clone()));
                processed_transaction_ids.insert(transaction.id);
            }
        } else {
            // Check if this transaction is a competitor for another transaction
            let is_competitor = inmempool_transactions
                .iter()
                .any(|tx| tx.cancelled_by_transaction_id.as_ref() == Some(&transaction.id));

            if !is_competitor {
                // This is a standalone transaction with no competition
                competitive_queue.push_back(CompetitiveTransaction::new(transaction.clone()));
                processed_transaction_ids.insert(transaction.id);
            }
            // If it is a competitor, it will be processed when we encounter the original
        }
    }

    info!(
        "Reconstructed {} competitive transactions for relayer {}",
        competitive_queue.len(),
        relayer_id
    );

    Ok(competitive_queue)
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

/// Imports private keys as relayers during startup
async fn import_private_keys_as_relayers(
    db: &PostgresClient,
    providers: &Vec<EvmProvider>,
    network_configs: &Vec<crate::yaml::NetworkSetupConfig>,
    global_signing_provider: Option<&crate::yaml::SigningProvider>,
) -> Result<(), StartTransactionsQueuesError> {
    use crate::relayer::CreateRelayerMode;

    for config in network_configs {
        // Find provider for this network
        let provider = match find_provider_for_chain_id(providers, &config.chain_id).await {
            Some(p) => p,
            None => {
                warn!("No provider found for chain {} during private key import", config.chain_id);
                continue;
            }
        };

        // Determine which signing provider to use (network-level or global)
        let signing_provider = if let Some(ref signing_key) = config.signing_provider {
            signing_key
        } else if let Some(global_provider) = global_signing_provider {
            global_provider
        } else {
            // No signing provider configured for this network, skip
            continue;
        };

        // Check if this signing provider has private keys
        if let Some(ref private_keys) = signing_provider.private_keys {
            info!(
                "Importing {} private keys as relayers for network {} (chain_id: {})",
                private_keys.len(),
                config.name,
                config.chain_id
            );

            let mut imported_relayers = Vec::new();

            for (index, _private_key_config) in private_keys.iter().enumerate() {
                let relayer_name = format!("{}-pk-{}", config.name, index);

                // Use negative wallet indexes for private keys to avoid conflicts with mnemonic wallets
                // Private key index 0 becomes wallet_index -1, index 1 becomes -2, etc.
                let private_key_wallet_index = -((index + 1) as i32);
                match provider.get_address(index as u32).await {
                    Ok(address) => {
                        // Check if a relayer with this address already exists on this chain
                        if let Ok(Some(existing_relayer)) =
                            db.get_relayer_by_address(&address, &config.chain_id).await
                        {
                            info!("Relayer for private key {} already exists on chain {} with address {} (relayer_id: {}), skipping import", 
                                  index, config.chain_id, address, existing_relayer.id);
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get address for private key {}: {}", index, e);
                        continue;
                    }
                }

                // Create the relayer using PrivateKeyImport mode with negative wallet index
                match db
                    .create_relayer(
                        &relayer_name,
                        &config.chain_id,
                        provider,
                        CreateRelayerMode::PrivateKeyImport(private_key_wallet_index),
                    )
                    .await
                {
                    Ok(relayer) => {
                        info!("Successfully imported private key {} as relayer {} with address {} (relayer_id: {}) on chain {}", 
                              index, relayer.name, relayer.address, relayer.id, config.chain_id);
                        imported_relayers.push((relayer.id, relayer.address));
                    }
                    Err(e) => {
                        warn!("Failed to import private key {} as relayer: {}", index, e);
                    }
                }
            }

            if !imported_relayers.is_empty() {
                info!(
                    "Completed importing {} new relayers for network {} (chain_id: {}): {:?}",
                    imported_relayers.len(),
                    config.name,
                    config.chain_id,
                    imported_relayers
                        .iter()
                        .map(|(id, addr)| format!("{}:{}", id, addr))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    Ok(())
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

    #[error("Failed to import private keys as relayers: {0}")]
    PrivateKeyImportError(#[from] crate::relayer::CreateRelayerError),
}

#[allow(clippy::too_many_arguments)]
pub async fn startup_transactions_queues(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    safe_proxy_manager: Arc<SafeProxyManager>,
    network_configs: Arc<Vec<crate::yaml::NetworkSetupConfig>>,
    global_signing_provider: Option<Arc<crate::yaml::SigningProvider>>,
) -> Result<Arc<Mutex<TransactionsQueues>>, StartTransactionsQueuesError> {
    let postgres = PostgresClient::new()
        .await
        .map_err(StartTransactionsQueuesError::DatabaseConnectionError)?;

    // Import private keys as relayers if configured
    import_private_keys_as_relayers(
        &postgres,
        &providers,
        &network_configs,
        global_signing_provider.as_deref(),
    )
    .await?;

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
                    repopulate_transaction_queue(&postgres, &relayer_id, &TransactionStatus::MINED)
                        .await?;

                let network_config =
                    network_configs.iter().find(|config| config.chain_id == relayer.chain_id);

                let gas_bump_config = network_config
                    .map(|config| config.gas_bump_blocks_every.clone())
                    .unwrap_or_default();

                let max_gas_price_multiplier =
                    network_config.map(|config| config.max_gas_price_multiplier).unwrap_or(2);

                transaction_relayer_setups.push(TransactionRelayerSetup::new(
                    relayer,
                    evm_provider,
                    repopulate_transaction_queue(
                        &postgres,
                        &relayer_id,
                        &TransactionStatus::PENDING,
                    )
                    .await?,
                    repopulate_competitive_transaction_queue(&postgres, &relayer_id).await?,
                    mined_transactions
                        .into_iter()
                        .map(|transaction| (transaction.id, transaction))
                        .collect(),
                    gas_bump_config,
                    max_gas_price_multiplier,
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
