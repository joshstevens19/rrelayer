use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use alloy::{
    consensus::TypedTransaction,
    transports::{RpcError, TransportErrorKind},
};
use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Error types for transaction queues operations.
#[derive(Error, Debug)]
pub enum TransactionsQueuesError {
    #[error("Wallet or provider error: {0}")]
    WalletOrProvider(#[from] WalletOrProviderError),
    #[error("Database connection error: {0}")]
    DatabaseConnection(#[from] PostgresConnectionError),
}

use super::{
    start::spawn_processing_tasks_for_relayer,
    transactions_queue::TransactionsQueue,
    types::{
        AddTransactionError, CancelTransactionError, EditableTransactionType,
        ProcessInmempoolStatus, ProcessInmempoolTransactionError, ProcessMinedStatus,
        ProcessMinedTransactionError, ProcessPendingStatus, ProcessPendingTransactionError,
        ProcessResult, ReplaceTransactionError, TransactionRelayerSetup, TransactionToSend,
        TransactionsQueueSetup,
    },
};
use crate::transaction::api::RelayTransactionRequest;
use crate::transaction::queue_system::types::SendTransactionGasPriceError;
use crate::transaction::types::{TransactionConversionError, TransactionSpeed};
use crate::{
    gas::{BlobGasOracleCache, BlobGasPriceResult, GasLimit, GasOracleCache, GasPriceResult},
    network,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    relayer::RelayerId,
    safe_proxy::SafeProxyManager,
    shared::{
        cache::Cache,
        common_types::{EvmAddress, WalletOrProviderError},
    },
    transaction::{
        cache::invalidate_transaction_no_state_cache,
        nonce_manager::NonceManager,
        queue_system::types::TransactionQueueSendTransactionError,
        types::{Transaction, TransactionData, TransactionId, TransactionStatus, TransactionValue},
    },
    webhooks::WebhookManager,
};

/// Container for managing multiple transaction queues across different relayers.
///
/// This struct coordinates transaction processing for all active relayers,
/// providing centralized access to individual queue operations and shared resources.
pub struct TransactionsQueues {
    pub queues: HashMap<RelayerId, Arc<Mutex<TransactionsQueue>>>,
    pub relayer_block_times_ms: HashMap<RelayerId, u64>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    db: PostgresClient,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    safe_proxy_manager: Option<SafeProxyManager>,
}

impl TransactionsQueues {
    /// Creates a new TransactionsQueues instance with the given relayer setups.
    pub async fn new(
        setups: Vec<TransactionRelayerSetup>,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
        blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
        cache: Arc<Cache>,
        webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
        safe_proxy_manager: Option<SafeProxyManager>,
    ) -> Result<Self, TransactionsQueuesError> {
        let mut queues = HashMap::new();
        let mut relayer_block_times_ms = HashMap::new();

        for setup in setups {
            let current_nonce = setup.evm_provider.get_nonce(&setup.relayer.wallet_index).await?;

            relayer_block_times_ms.insert(setup.relayer.id, setup.evm_provider.blocks_every);

            queues.insert(
                setup.relayer.id,
                Arc::new(Mutex::new(TransactionsQueue::new(
                    TransactionsQueueSetup::new(
                        setup.relayer,
                        setup.evm_provider,
                        NonceManager::new(current_nonce),
                        setup.pending_transactions,
                        setup.inmempool_transactions,
                        setup.mined_transactions,
                        safe_proxy_manager.clone(),
                    ),
                    gas_oracle_cache.clone(),
                    blob_gas_oracle_cache.clone(),
                ))),
            );
        }

        Ok(Self {
            queues,
            relayer_block_times_ms,
            gas_oracle_cache,
            blob_gas_oracle_cache,
            db: PostgresClient::new().await?,
            cache,
            webhook_manager,
            safe_proxy_manager,
        })
    }

    /// Retrieves a transaction queue for the specified relayer.
    pub fn get_transactions_queue(
        &self,
        relayer_id: &RelayerId,
    ) -> Option<Arc<Mutex<TransactionsQueue>>> {
        self.queues.get(relayer_id).cloned()
    }

    /// Retrieves a transaction queue for the specified relayer.
    pub fn get_transactions_queue_unsafe(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<Arc<Mutex<TransactionsQueue>>, String> {
        self.queues
            .get(relayer_id)
            .cloned()
            .ok_or_else(|| format!("transactions queue does not exist for relayer: {}", relayer_id))
    }

    /// Removes a transaction queue for the specified relayer.
    pub async fn delete_queue(&mut self, relayer_id: &RelayerId) {
        self.queues.remove(relayer_id);
    }

    /// Invalidates the cache entry for a specific transaction.
    async fn invalidate_transaction_cache(&self, id: &TransactionId) {
        invalidate_transaction_no_state_cache(&self.cache, id).await;
    }

    /// Returns the count of pending transactions for a specific relayer.
    pub async fn pending_transactions_count(&self, relayer_id: &RelayerId) -> usize {
        if let Some(queue_arc) = self.get_transactions_queue(relayer_id) {
            let queue = queue_arc.lock().await;
            queue.get_pending_transaction_count().await
        } else {
            0
        }
    }

    /// Returns the count of in-mempool transactions for a specific relayer.
    pub async fn inmempool_transactions_count(&self, relayer_id: &RelayerId) -> usize {
        if let Some(queue_arc) = self.get_transactions_queue(relayer_id) {
            let queue = queue_arc.lock().await;
            queue.get_inmempool_transaction_count().await
        } else {
            0
        }
    }

    /// Adds a new relayer and its transaction queue to the system.
    pub async fn add_new_relayer(
        &mut self,
        setup: TransactionsQueueSetup,
        queues_arc: Arc<Mutex<TransactionsQueues>>,
    ) -> Result<(), WalletOrProviderError> {
        let current_nonce = setup.evm_provider.get_nonce(&setup.relayer.wallet_index).await?;
        let relayer_id = setup.relayer.id;

        self.queues.insert(
            relayer_id,
            Arc::new(Mutex::new(TransactionsQueue::new(
                TransactionsQueueSetup::new(
                    setup.relayer,
                    setup.evm_provider,
                    NonceManager::new(current_nonce),
                    VecDeque::new(),
                    VecDeque::new(),
                    HashMap::new(),
                    self.safe_proxy_manager.clone(),
                ),
                self.gas_oracle_cache.clone(),
                self.blob_gas_oracle_cache.clone(),
            ))),
        );

        spawn_processing_tasks_for_relayer(queues_arc, &relayer_id).await;

        Ok(())
    }

    fn expires_at(&self) -> DateTime<Utc> {
        Utc::now() + chrono::Duration::hours(12)
    }

    /// Checks if a transaction has expired.
    fn has_expired(&self, transaction: &Transaction) -> bool {
        transaction.expires_at < Utc::now()
    }

    /// Converts a transaction to a no-op transaction.
    fn transaction_to_noop(
        &self,
        transactions_queue: &mut TransactionsQueue,
        transaction: &mut Transaction,
    ) {
        transaction.to = transactions_queue.relay_address();
        transaction.value = TransactionValue::zero();
        transaction.data = TransactionData::empty();
        transaction.gas_limit = Some(GasLimit::new(21000_u128));
        transaction.is_noop = true;
        transaction.speed = TransactionSpeed::Fast;
    }

    /// Replaces the content of an existing transaction with new parameters.
    fn transaction_replace(
        &self,
        current_transaction: &mut Transaction,
        replace_with: &RelayTransactionRequest,
    ) {
        // TODO: blobs
        current_transaction.to = replace_with.to;
        current_transaction.data = replace_with.data.clone();
        current_transaction.value = replace_with.value;
        current_transaction.is_noop = current_transaction.from == current_transaction.to;
        current_transaction.gas_limit = None;
        current_transaction.external_id = replace_with.external_id.clone();
    }

    /// Checks if a relayer is allowed to send transactions to a specific address.
    async fn relayer_allowed_to_send_transaction_to(
        &self,
        relayer_id: &RelayerId,
        to: &EvmAddress,
    ) -> Result<bool, PostgresError> {
        let relayer = self.db.is_relayer_allowlist_address(relayer_id, to).await?;
        Ok(relayer)
    }

    /// Computes gas prices for a transaction based on its type.
    async fn compute_transaction_gas_prices(
        transactions_queue: &TransactionsQueue,
        transaction: &Transaction,
        speed: &TransactionSpeed,
    ) -> Result<(GasPriceResult, Option<BlobGasPriceResult>), SendTransactionGasPriceError> {
        let gas_price = transactions_queue.compute_gas_price_for_transaction(speed, None).await?;

        let blob_gas_price = if transaction.is_blob_transaction() {
            Some(transactions_queue.compute_blob_gas_price_for_transaction(speed, &None).await?)
        } else {
            None
        };

        Ok((gas_price, blob_gas_price))
    }

    /// Creates a typed transaction request for gas estimation or sending.
    fn create_typed_transaction(
        transactions_queue: &TransactionsQueue,
        transaction: &Transaction,
        gas_price: &GasPriceResult,
        blob_gas_price: Option<&BlobGasPriceResult>,
        gas_limit: GasLimit,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        if transaction.is_blob_transaction() {
            Ok(transaction.to_blob_typed_transaction_with_gas_limit(
                Some(gas_price),
                blob_gas_price,
                Some(gas_limit),
            )?)
        } else if transactions_queue.is_legacy_transactions() {
            Ok(transaction
                .to_legacy_typed_transaction_with_gas_limit(Some(gas_price), Some(gas_limit))?)
        } else {
            Ok(transaction
                .to_eip1559_typed_transaction_with_gas_limit(Some(gas_price), Some(gas_limit))?)
        }
    }

    /// Estimates gas limit for a transaction and validates it via simulation.
    async fn estimate_and_validate_gas(
        transactions_queue: &mut TransactionsQueue,
        transaction: &Transaction,
        gas_price: &GasPriceResult,
        blob_gas_price: Option<&BlobGasPriceResult>,
    ) -> Result<GasLimit, AddTransactionError> {
        // Use a reasonable temporary limit for gas estimation
        const TEMP_GAS_LIMIT: u128 = 1_000_000;
        let temp_gas_limit = GasLimit::new(TEMP_GAS_LIMIT);

        let current_onchain_nonce = transactions_queue.get_nonce().await.map_err(|e| {
            AddTransactionError::CouldNotGetCurrentOnChainNonce(transaction.relayer_id, e)
        })?;

        let mut estimation_transaction = transaction.clone();
        estimation_transaction.nonce = current_onchain_nonce;

        let temp_transaction_request = Self::create_typed_transaction(
            transactions_queue,
            &estimation_transaction,
            gas_price,
            blob_gas_price,
            temp_gas_limit,
        )?;

        let estimated_gas_limit = transactions_queue
            .estimate_gas(&temp_transaction_request, transaction.is_noop)
            .await
            .map_err(|e| {
                AddTransactionError::TransactionEstimateGasError(transaction.relayer_id, e)
            })?;

        let relayer_balance = transactions_queue.get_balance().await.map_err(|e| {
            AddTransactionError::TransactionEstimateGasError(transaction.relayer_id, e)
        })?;

        let gas_cost = estimated_gas_limit.into_inner() * gas_price.legacy_gas_price().into_u128();
        let total_required =
            transaction.value.into_inner() + alloy::primitives::U256::from(gas_cost);

        if relayer_balance < total_required {
            error!(
                "Insufficient balance for relayer {}: has {}, needs {}",
                transaction.relayer_id, relayer_balance, total_required
            );
            return Err(AddTransactionError::TransactionEstimateGasError(
                transaction.relayer_id,
                RpcError::Transport(TransportErrorKind::Custom(
                    "Insufficient funds for gas * price + value".to_string().into(),
                )),
            ));
        }

        Ok(estimated_gas_limit)
    }

    /// Adds a new transaction to the specified relayer's queue.
    pub async fn add_transaction(
        &mut self,
        relayer_id: &RelayerId,
        transaction_to_send: &TransactionToSend,
    ) -> Result<Transaction, AddTransactionError> {
        let expires_at = self.expires_at();

        let queue_arc = self
            .get_transactions_queue(relayer_id)
            .ok_or(AddTransactionError::RelayerNotFound(*relayer_id))?;

        let mut transactions_queue = queue_arc.lock().await;

        if transactions_queue.is_paused() {
            return Err(AddTransactionError::RelayerIsPaused(*relayer_id));
        }

        // Check if the network is disabled using cache to avoid DB lookup
        let chain_id = transactions_queue.chain_id();
        if let Some(networks) = network::get_networks_cache(&self.cache).await {
            if let Some(network) = networks.iter().find(|n| n.chain_id == chain_id) {
                if network.disabled {
                    return Err(AddTransactionError::NetworkDisabled(chain_id));
                }
            }
            // If network not found in the cache, optimistically assume it's enabled (new network)
        }
        // If cache returns None (cache not available), we allow the transaction through
        // to maintain system availability. The cache is refreshed every 10 minutes by a background task.
        // that said, it pushes the cache on new network creations and disabled and enabled

        if transactions_queue.is_allowlisted_only()
            && !self
                .relayer_allowed_to_send_transaction_to(relayer_id, &transaction_to_send.to)
                .await
                .map_err(AddTransactionError::CouldNotReadAllowlistsFromDb)?
        {
            return Err(AddTransactionError::RelayerNotAllowedToSendTransactionTo(
                *relayer_id,
                transaction_to_send.to,
            ));
        }

        let assigned_nonce = transactions_queue.nonce_manager.get_and_increment().await;

        let mut transaction = Transaction {
            id: transaction_to_send.id,
            relayer_id: *relayer_id,
            to: transaction_to_send.to,
            from: transactions_queue.relay_address(),
            value: transaction_to_send.value,
            data: transaction_to_send.data.clone(),
            nonce: assigned_nonce,
            gas_limit: None,
            status: TransactionStatus::Pending,
            blobs: transaction_to_send.blobs.clone(),
            chain_id: transactions_queue.chain_id(),
            known_transaction_hash: None,
            queued_at: Utc::now(),
            expires_at,
            sent_at: None,
            mined_at: None,
            mined_at_block_number: None,
            confirmed_at: None,
            speed: transaction_to_send.speed.clone(),
            sent_with_max_priority_fee_per_gas: None,
            sent_with_max_fee_per_gas: None,
            is_noop: false,
            sent_with_gas: None,
            sent_with_blob_gas: None,
            external_id: transaction_to_send.external_id.clone(),
        };

        let (gas_price, blob_gas_price) = Self::compute_transaction_gas_prices(
            &transactions_queue,
            &transaction,
            &transaction_to_send.speed,
        )
        .await?;

        let estimated_gas_limit = Self::estimate_and_validate_gas(
            &mut transactions_queue,
            &transaction,
            &gas_price,
            blob_gas_price.as_ref(),
        )
        .await;

        let estimated_gas_limit = match estimated_gas_limit {
            Ok(limit) => limit,
            Err(err) => {
                self.db
                    .transaction_failed_on_send(
                        relayer_id,
                        &transaction,
                        "Failed to send transaction as always failing on gas estimation",
                    )
                    .await
                    .map_err(AddTransactionError::CouldNotSaveTransactionDb)?;

                self.invalidate_transaction_cache(&transaction.id).await;
                return Err(err);
            }
        };

        transaction.gas_limit = Some(estimated_gas_limit);

        let transaction_request = Self::create_typed_transaction(
            &transactions_queue,
            &transaction,
            &gas_price,
            blob_gas_price.as_ref(),
            estimated_gas_limit,
        )?;

        transaction.known_transaction_hash =
            Some(transactions_queue.compute_tx_hash(&transaction_request).await?);

        self.db
            .save_transaction(relayer_id, &transaction)
            .await
            .map_err(AddTransactionError::CouldNotSaveTransactionDb)?;

        transactions_queue.add_pending_transaction(transaction.clone()).await;
        // Nonce already incremented atomically above - no need for separate increase call
        self.invalidate_transaction_cache(&transaction.id).await;

        if let Some(webhook_manager) = &self.webhook_manager {
            let webhook_manager = webhook_manager.clone();
            let transaction_clone = transaction.clone();
            tokio::spawn(async move {
                let webhook_manager = webhook_manager.lock().await;
                webhook_manager.on_transaction_queued(&transaction_clone).await;
            });
        }

        Ok(transaction)
    }

    /// Cancels an existing transaction by converting it to a no-op.
    pub async fn cancel_transaction(
        &mut self,
        transaction: &Transaction,
    ) -> Result<bool, CancelTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(&transaction.relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;
            if transactions_queue.is_paused() {
                return Err(CancelTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if let Some(mut result) =
                transactions_queue.get_editable_transaction_by_id(&transaction.id).await
            {
                match result.type_name {
                    EditableTransactionType::Pending => {
                        info!("cancel_transaction: converting to noop - pending");
                        self.transaction_to_noop(&mut transactions_queue, &mut result.transaction);
                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let transaction_clone = result.transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager.on_transaction_cancelled(&transaction_clone).await;
                            });
                        }

                        Ok(true)
                    }
                    EditableTransactionType::Inmempool => {
                        info!("cancel_transaction: converting to noop - inmempool");
                        self.transaction_to_noop(&mut transactions_queue, &mut result.transaction);
                        info!(
                            "cancel_transaction: sending noop - inmempool {:?}",
                            result.transaction
                        );

                        let transaction_sent = transactions_queue
                            .send_transaction(&mut self.db, &mut result.transaction)
                            .await?;

                        transactions_queue
                            .update_inmempool_transaction_noop(&transaction.id, &transaction_sent)
                            .await;

                        // TODO: not sure we need?
                        result.transaction.known_transaction_hash = Some(transaction_sent.hash);

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let transaction_clone = result.transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager.on_transaction_cancelled(&transaction_clone).await;
                            });
                        }

                        Ok(true)
                    }
                }
            } else {
                Ok(false)
            }
        } else {
            Err(CancelTransactionError::RelayerNotFound(transaction.relayer_id))
        }
    }

    /// Replaces an existing transaction with new parameters.
    /// TODO: look at the nonce management for replace to predict the new gas limit
    pub async fn replace_transaction(
        &mut self,
        transaction: &Transaction,
        replace_with: &RelayTransactionRequest,
    ) -> Result<bool, ReplaceTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(&transaction.relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;

            if transactions_queue.is_paused() {
                return Err(ReplaceTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if transactions_queue.is_allowlisted_only()
                && !self
                    .relayer_allowed_to_send_transaction_to(
                        &transaction.relayer_id,
                        &transaction.to,
                    )
                    .await
                    .map_err(ReplaceTransactionError::CouldNotReadAllowlistsFromDb)?
            {
                return Err(ReplaceTransactionError::RelayerNotAllowedToSendTransactionTo(
                    transaction.relayer_id,
                    transaction.to,
                ));
            }

            if let Some(mut result) =
                transactions_queue.get_editable_transaction_by_id(&transaction.id).await
            {
                match result.type_name {
                    EditableTransactionType::Pending => {
                        let original_transaction = result.transaction.clone();
                        self.transaction_replace(&mut result.transaction, replace_with);
                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let new_transaction = result.transaction.clone();
                            let original_transaction_clone = original_transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_replaced(
                                        &new_transaction,
                                        &original_transaction_clone,
                                    )
                                    .await;
                            });
                        }

                        Ok(true)
                    }
                    EditableTransactionType::Inmempool => {
                        let original_transaction = result.transaction.clone();
                        self.transaction_replace(&mut result.transaction, replace_with);

                        let transaction_sent = transactions_queue
                            .send_transaction(&mut self.db, &mut result.transaction)
                            .await?;

                        transactions_queue
                            .update_inmempool_transaction_replaced(
                                &transaction.id,
                                &transaction_sent,
                                &result.transaction,
                            )
                            .await;

                        self.db.transaction_update(&result.transaction).await?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let new_transaction = result.transaction.clone();
                            let original_transaction_clone = original_transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_replaced(
                                        &new_transaction,
                                        &original_transaction_clone,
                                    )
                                    .await;
                            });
                        }

                        Ok(true)
                    }
                }
            } else {
                Ok(false)
            }
        } else {
            Err(ReplaceTransactionError::TransactionNotFound(transaction.id))
        }
    }

    /// Processes a single pending transaction for the specified relayer.
    pub async fn process_single_pending(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessPendingStatus>, ProcessPendingTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;

            if transactions_queue.is_paused() {
                return Ok(ProcessResult::<ProcessPendingStatus>::other(
                    ProcessPendingStatus::RelayerPaused,
                    Some(&30000), // relayer paused we will wait 30 seconds to get new stuff
                ));
            }

            if let Some(mut transaction) = transactions_queue.get_next_pending_transaction().await {
                if self.has_expired(&transaction) {
                    self.transaction_to_noop(&mut transactions_queue, &mut transaction);
                }

                match transactions_queue.send_transaction(&mut self.db, &mut transaction).await {
                    Ok(transaction_sent) => {
                        transactions_queue.move_pending_to_inmempool(&transaction_sent).await?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let sent_transaction = Transaction {
                                status: TransactionStatus::Inmempool,
                                known_transaction_hash: Some(transaction_sent.hash),
                                sent_at: Some(Utc::now()),
                                ..transaction
                            };
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager.on_transaction_sent(&sent_transaction).await;
                            });
                        }
                    }
                    Err(e) => {
                        return match e {
                            TransactionQueueSendTransactionError::GasPriceTooHigh => {
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::GasPriceTooHigh,
                                    self.relayer_block_times_ms.get(relayer_id), /* gas to high check back on next block */
                                ))
                            }
                            TransactionQueueSendTransactionError::GasCalculationError => {
                                Err(ProcessPendingTransactionError::GasCalculationError(
                                    *relayer_id,
                                    transaction.clone(),
                                ))
                            }
                            TransactionQueueSendTransactionError::TransactionEstimateGasError(
                                error,
                            ) => {
                                self.db
                                    .update_transaction_failed(&transaction.id, &error.to_string())
                                    .await
                                    .map_err(ProcessPendingTransactionError::DbError)?;

                                transactions_queue.move_next_pending_to_failed().await;

                                self.invalidate_transaction_cache(&transaction.id).await;

                                Err(ProcessPendingTransactionError::TransactionEstimateGasError(
                                    error,
                                ))
                            }
                            TransactionQueueSendTransactionError::TransactionSendError(error) => {
                                // if it fails to send it means RPC node must be down as we
                                // override the gas limits
                                // this enforces that the tx
                                // will go through even if estimate gas is wrong
                                // another point could be low on funds and rejecting the queue
                                // here seems an odd stance
                                // as it could
                                // be a temp issue
                                Err(ProcessPendingTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::TransactionSendError(
                                        error,
                                    ),
                                ))
                            }
                            TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(
                                error,
                            ) => {
                                // just keep the transaction in pending state as could be a bad
                                // db connection or temp
                                // outage
                                Err(ProcessPendingTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(error),
                                ))
                            }
                            TransactionQueueSendTransactionError::SendTransactionGasPriceError(
                                error,
                            ) => {
                                // should never happen if it does something internal is wrong,
                                // and we don't want to
                                // continue processing the queue
                                // it can stay in a loop forever, so we don't fail pending
                                // transactions
                                Err(ProcessPendingTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::SendTransactionGasPriceError(error),
                                ))
                            }
                            TransactionQueueSendTransactionError::TransactionConversionError(
                                error,
                            ) => {
                                self.db
                                    .update_transaction_failed(&transaction.id, &error)
                                    .await
                                    .map_err(ProcessPendingTransactionError::DbError)?;

                                transactions_queue.move_next_pending_to_failed().await;

                                self.invalidate_transaction_cache(&transaction.id).await;

                                Err(ProcessPendingTransactionError::TransactionEstimateGasError(
                                    RpcError::Transport(TransportErrorKind::Custom(error.into())),
                                ))
                            }
                            TransactionQueueSendTransactionError::SafeProxyError(error) => {
                                self.db
                                    .update_transaction_failed(&transaction.id, &error.to_string())
                                    .await
                                    .map_err(ProcessPendingTransactionError::DbError)?;

                                transactions_queue.move_next_pending_to_failed().await;

                                self.invalidate_transaction_cache(&transaction.id).await;

                                Err(ProcessPendingTransactionError::TransactionEstimateGasError(
                                    RpcError::Transport(TransportErrorKind::Custom(error.into())),
                                ))
                            }
                        };
                    }
                }

                Ok(ProcessResult::<ProcessPendingStatus>::success())
            } else {
                Ok(ProcessResult::<ProcessPendingStatus>::other(
                    ProcessPendingStatus::NoPendingTransactions,
                    Default::default(),
                ))
            }
        } else {
            Err(ProcessPendingTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
        }
    }

    /// Processes a single in-mempool transaction for the specified relayer.
    pub async fn process_single_inmempool(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessInmempoolStatus>, ProcessInmempoolTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;

            if let Some(mut transaction) = transactions_queue.get_next_inmempool_transaction().await
            {
                if let Some(known_transaction_hash) = transaction.known_transaction_hash {
                    match transactions_queue.get_receipt(&known_transaction_hash).await {
                        Ok(Some(receipt)) => {
                            let status = transactions_queue
                                .move_inmempool_to_mining(&transaction.id, &receipt)
                                .await.map_err(ProcessInmempoolTransactionError::MoveInmempoolTransactionToMinedError)?;

                            match status {
                                TransactionStatus::Mined => {
                                    self.db
                                        .transaction_mined(&transaction, &receipt)
                                        .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Mined, e))?;
                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    if let Some(webhook_manager) = &self.webhook_manager {
                                        let webhook_manager = webhook_manager.clone();
                                        let mined_transaction = Transaction {
                                            status: TransactionStatus::Mined,
                                            mined_at: Some(Utc::now()),
                                            ..transaction
                                        };
                                        let receipt_clone = receipt.clone();
                                        tokio::spawn(async move {
                                            let webhook_manager = webhook_manager.lock().await;
                                            webhook_manager
                                                .on_transaction_mined(
                                                    &mined_transaction,
                                                    &receipt_clone,
                                                )
                                                .await;
                                        });
                                    }
                                }
                                TransactionStatus::Expired => {
                                    self.db.transaction_expired(&transaction.id).await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Expired, e))?;
                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    if let Some(webhook_manager) = &self.webhook_manager {
                                        let webhook_manager = webhook_manager.clone();
                                        let expired_transaction = Transaction {
                                            status: TransactionStatus::Expired,
                                            ..transaction
                                        };
                                        tokio::spawn(async move {
                                            let webhook_manager = webhook_manager.lock().await;
                                            webhook_manager
                                                .on_transaction_expired(&expired_transaction)
                                                .await;
                                        });
                                    }
                                }
                                TransactionStatus::Failed => {
                                    self.db
                                        .update_transaction_failed(&transaction.id, "Failed onchain")
                                        .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Failed, e))?;
                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    if let Some(webhook_manager) = &self.webhook_manager {
                                        let webhook_manager = webhook_manager.clone();
                                        let failed_transaction = Transaction {
                                            status: TransactionStatus::Failed,
                                            ..transaction
                                        };
                                        tokio::spawn(async move {
                                            let webhook_manager = webhook_manager.lock().await;
                                            webhook_manager
                                                .on_transaction_failed(&failed_transaction)
                                                .await;
                                        });
                                    }
                                }
                                _ => {}
                            }

                            Ok(ProcessResult::<ProcessInmempoolStatus>::success())
                        }
                        Ok(None) => {
                            if let Some(sent_at) = transaction.sent_at {
                                let elapsed = Utc::now() - sent_at;
                                if transactions_queue.should_bump_gas(
                                    elapsed.num_milliseconds() as u64,
                                    &transaction.speed,
                                ) {
                                    let transaction_sent = transactions_queue
                                        .send_transaction(&mut self.db, &mut transaction)
                                        .await?;

                                    // Update the actual transaction in the inmempool queue
                                    transactions_queue
                                        .update_inmempool_transaction_gas(&transaction_sent)
                                        .await;

                                    // Update the local transaction with the new gas values so subsequent bumps work correctly
                                    transaction.known_transaction_hash =
                                        Some(transaction_sent.hash);
                                    transaction.sent_with_max_fee_per_gas =
                                        Some(transaction_sent.sent_with_gas.max_fee);
                                    transaction.sent_with_max_priority_fee_per_gas =
                                        Some(transaction_sent.sent_with_gas.max_priority_fee);
                                    transaction.sent_with_gas =
                                        Some(transaction_sent.sent_with_gas.clone());
                                    transaction.sent_at = Some(Utc::now());

                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                        ProcessInmempoolStatus::GasIncreased,
                                        Default::default(),
                                    ));
                                }
                            }

                            Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                ProcessInmempoolStatus::StillInmempool,
                                Some(&500), // recheck again in 500ms
                            ))
                        }
                        Err(e) => {
                            Err(ProcessInmempoolTransactionError::CouldNotGetTransactionReceipt(
                                *relayer_id,
                                transaction.clone(),
                                e,
                            ))
                        }
                    }
                } else {
                    Err(ProcessInmempoolTransactionError::UnknownTransactionHash(
                        *relayer_id,
                        transaction.clone(),
                    ))
                }
            } else {
                Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                    ProcessInmempoolStatus::NoInmempoolTransactions,
                    Default::default(),
                ))
            }
        } else {
            Err(ProcessInmempoolTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
        }
    }

    /// Processes a single mined transaction for the specified relayer.
    pub async fn process_single_mined(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessMinedStatus>, ProcessMinedTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;

            if let Some(transaction) = transactions_queue.get_next_mined_transaction().await {
                if let Some(mined_at) = transaction.mined_at {
                    let elapsed = Utc::now() - mined_at;
                    if transactions_queue.in_confirmed_range(elapsed.num_milliseconds() as u64) {
                        let receipt = if let Some(tx_hash) = transaction.known_transaction_hash {
                            transactions_queue
                                .get_receipt(&tx_hash)
                                .await
                                .map_err(|e| {
                                    ProcessMinedTransactionError::CouldNotGetTransactionReceipt(
                                        *relayer_id,
                                        transaction.clone(),
                                        e,
                                    )
                                })?
                                .ok_or(
                                    ProcessMinedTransactionError::CouldNotGetTransactionReceipt(
                                        *relayer_id,
                                        transaction.clone(),
                                        RpcError::Transport(TransportErrorKind::Custom(
                                            "No receipt".to_string().into(),
                                        )),
                                    ),
                                )?
                        } else {
                            return Err(
                                ProcessMinedTransactionError::CouldNotGetTransactionReceipt(
                                    *relayer_id,
                                    transaction.clone(),
                                    RpcError::Transport(TransportErrorKind::Custom(
                                        "Transaction hash not found".to_string().into(),
                                    )),
                                ),
                            );
                        };

                        self.db.transaction_confirmed(&transaction.id).await.map_err(|e| {
                            ProcessMinedTransactionError::TransactionConfirmedNotSaveToDatabase(
                                *relayer_id,
                                transaction.clone(),
                                e,
                            )
                        })?;

                        transactions_queue.move_mining_to_confirmed(&transaction.id).await;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let confirmed_transaction = Transaction {
                                status: TransactionStatus::Confirmed,
                                confirmed_at: Some(Utc::now()),
                                ..transaction
                            };
                            let receipt_clone = receipt.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_confirmed(
                                        &confirmed_transaction,
                                        &receipt_clone,
                                    )
                                    .await;
                            });
                        }

                        return Ok(ProcessResult::<ProcessMinedStatus>::success());
                    }

                    Ok(ProcessResult::<ProcessMinedStatus>::other(
                        ProcessMinedStatus::NotConfirmedYet,
                        Default::default(),
                    ))
                } else {
                    Err(ProcessMinedTransactionError::NoMinedAt(*relayer_id, transaction.clone()))
                }
            } else {
                Ok(ProcessResult::<ProcessMinedStatus>::other(
                    ProcessMinedStatus::NoMinedTransactions,
                    Default::default(),
                ))
            }
        } else {
            Err(ProcessMinedTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
        }
    }
}
