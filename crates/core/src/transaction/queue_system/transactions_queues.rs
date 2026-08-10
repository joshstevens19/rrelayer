use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use alloy::{
    consensus::TypedTransaction,
    network::AnyTransactionReceipt,
    transports::{RpcError, TransportErrorKind},
};
use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

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
    transactions_queue::{
        classify_send_error, InflightNonceHolder, SendErrorClass, TransactionsQueue,
    },
    types::{
        AddTransactionError, CancelTransactionError, CancelTransactionResult, CompetitionType,
        EditableTransactionType, ProcessInmempoolStatus, ProcessInmempoolTransactionError,
        ProcessMinedStatus, ProcessMinedTransactionError, ProcessPendingStatus,
        ProcessPendingTransactionError, ProcessResult, ReplaceTransactionError,
        ReplaceTransactionResult, SendTransactionAtNonceError, SendTransactionAtNonceResult,
        TransactionRelayerSetup, TransactionSentWithRelayer, TransactionToSend,
        TransactionsQueueSetup,
    },
};
use crate::transaction::api::RelayTransactionRequest;
use crate::transaction::queue_system::types::SendTransactionGasPriceError;
use crate::transaction::types::{
    GasPriceCeilingOutcome, TransactionBlob, TransactionConversionError, TransactionNonce,
    TransactionSpeed,
};
use crate::{
    gas::{
        BlobGasOracleCache, BlobGasPriceResult, GasLimit, GasOracleCache, GasPriceResult, MaxFee,
        MaxPriorityFee,
    },
    postgres::{PostgresClient, PostgresConnectionError},
    relayer::RelayerId,
    safe_proxy::SafeProxyManager,
    shared::{
        cache::Cache,
        common_types::{EvmAddress, WalletOrProviderError},
    },
    shutdown::enter_critical_operation,
    transaction::{
        cache::invalidate_transaction_no_state_cache,
        nonce_manager::NonceManager,
        queue_system::types::TransactionQueueSendTransactionError,
        types::{
            Transaction, TransactionData, TransactionHash, TransactionId, TransactionStatus,
            TransactionValue,
        },
    },
    webhooks::WebhookManager,
};

const SAME_NONCE_BUMP_DIVISOR: u128 = 5;
const MIN_SAME_NONCE_GAS_BUMP_WEI: u128 = 1_000_000_000;

fn bump_u128_for_same_nonce_competitor(value: u128) -> u128 {
    value
        .saturating_add(std::cmp::max(value / SAME_NONCE_BUMP_DIVISOR, MIN_SAME_NONCE_GAS_BUMP_WEI))
}

fn bump_max_fee_for_same_nonce_competitor(max_fee: MaxFee) -> MaxFee {
    MaxFee::new(bump_u128_for_same_nonce_competitor(max_fee.into_u128()))
}

fn bump_max_priority_fee_for_same_nonce_competitor(
    max_priority_fee: MaxPriorityFee,
) -> MaxPriorityFee {
    MaxPriorityFee::new(bump_u128_for_same_nonce_competitor(max_priority_fee.into_u128()))
}

/// True when a send-at-nonce payload is the canonical cancel shape - a value-0
/// self-send with no calldata and no blobs - and should clear the nonce through the
/// cancel machinery rather than replace it.
fn is_cancel_shaped(request: &RelayTransactionRequest, relayer_address: &EvmAddress) -> bool {
    request.to == *relayer_address
        && request.value.is_zero()
        && request.data == TransactionData::empty()
        && request.blobs.as_ref().is_none_or(|blobs| blobs.is_empty())
}

/// Resolves the expiry window for a transaction: the operator's global expiration
/// unless the request carries a SHORTER per-transaction expiry. The global setting
/// stays the operator ceiling - a per-transaction value can only tighten the window,
/// never extend it (longer or unrepresentable values clamp to the global window).
fn resolve_expiry_duration(
    global_expiration: chrono::Duration,
    expires_in_seconds: Option<u64>,
) -> chrono::Duration {
    let Some(seconds) = expires_in_seconds else {
        return global_expiration;
    };

    let per_transaction = i64::try_from(seconds).ok().and_then(chrono::Duration::try_seconds);

    match per_transaction {
        Some(duration) if duration < global_expiration => duration,
        _ => global_expiration,
    }
}

/// Guard rails for send-at-nonce: the nonce must still be in flight (not yet consumed
/// on chain) and held by a transaction a same-nonce replacement can act on. Returns
/// the holder to replace.
fn validate_exact_nonce_target(
    relayer_id: &RelayerId,
    nonce: TransactionNonce,
    onchain_nonce: TransactionNonce,
    holder: InflightNonceHolder,
) -> Result<Transaction, SendTransactionAtNonceError> {
    if nonce.into_inner() < onchain_nonce.into_inner() {
        return Err(SendTransactionAtNonceError::NonceAlreadyUsedOnchain(
            nonce,
            *relayer_id,
            onchain_nonce,
        ));
    }

    match holder {
        InflightNonceHolder::Pending(transaction)
        | InflightNonceHolder::InmempoolHead(transaction) => Ok(transaction),
        InflightNonceHolder::InmempoolBehindHead(_) => {
            Err(SendTransactionAtNonceError::NonceBehindInmempoolHead(nonce, *relayer_id))
        }
        InflightNonceHolder::NotFound => {
            Err(SendTransactionAtNonceError::NonceNotPending(nonce, *relayer_id))
        }
    }
}

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
    safe_proxy_manager: Arc<SafeProxyManager>,
}

impl TransactionsQueues {
    pub async fn new(
        setups: Vec<TransactionRelayerSetup>,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
        blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
        cache: Arc<Cache>,
        webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
        safe_proxy_manager: Arc<SafeProxyManager>,
    ) -> Result<Self, TransactionsQueuesError> {
        let mut queues = HashMap::new();
        let mut relayer_block_times_ms = HashMap::new();

        for setup in setups {
            let current_nonce = setup.evm_provider.get_nonce(&setup.relayer).await?;

            info!(
                "Startup nonce synchronization for relayer {} ({}): synchronizing nonce manager with on-chain nonce {}",
                setup.relayer.name, setup.relayer.id, current_nonce.into_inner()
            );

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
                        setup.gas_bump_config,
                        setup.max_gas_price_multiplier,
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
        let current_nonce = setup.evm_provider.get_nonce(&setup.relayer).await?;
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
                    setup.gas_bump_config,
                    setup.max_gas_price_multiplier,
                ),
                self.gas_oracle_cache.clone(),
                self.blob_gas_oracle_cache.clone(),
            ))),
        );

        spawn_processing_tasks_for_relayer(queues_arc, &relayer_id).await;

        Ok(())
    }

    /// The operator-wide expiration window: RRELAYER_TRANSACTION_EXPIRATION_SECONDS,
    /// defaulting to 12 hours. This is the CEILING for every transaction - a
    /// per-transaction expiry can only tighten it.
    fn global_expiration() -> chrono::Duration {
        std::env::var("RRELAYER_TRANSACTION_EXPIRATION_SECONDS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|seconds| *seconds > 0)
            // try_seconds guards against values chrono::Duration::seconds would panic on
            .and_then(chrono::Duration::try_seconds)
            .unwrap_or_else(|| chrono::Duration::hours(12))
    }

    /// Deadline for a transaction queued now: the global expiration unless the request
    /// carries a shorter per-transaction expiry.
    fn expires_at(&self, expires_in_seconds: Option<u64>) -> DateTime<Utc> {
        Utc::now() + resolve_expiry_duration(Self::global_expiration(), expires_in_seconds)
    }

    /// Computes gas prices for a transaction based on its type.
    async fn compute_transaction_gas_prices(
        transactions_queue: &TransactionsQueue,
        transaction: &Transaction,
        speed: &TransactionSpeed,
    ) -> Result<(GasPriceResult, Option<BlobGasPriceResult>), SendTransactionGasPriceError> {
        let blob_gas_price = if transaction.is_blob_transaction() {
            Some(transactions_queue.compute_blob_gas_price_for_transaction(speed, &None).await?)
        } else {
            None
        };

        let gas_price = transactions_queue.compute_gas_price_for_transaction(speed, None).await?;

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
        let expires_at = self.expires_at(transaction_to_send.expires_in_seconds);

        let queue_arc = self
            .get_transactions_queue(relayer_id)
            .ok_or(AddTransactionError::RelayerNotFound(*relayer_id))?;

        let mut transactions_queue = queue_arc.lock().await;

        if transactions_queue.is_paused() {
            return Err(AddTransactionError::RelayerIsPaused(*relayer_id));
        }

        // Check if this is a blob transaction and if the wallet manager supports blobs
        if transaction_to_send.blobs.is_some() && !transactions_queue.supports_blobs() {
            return Err(AddTransactionError::UnsupportedTransactionType {
                message: "EIP-4844 blob transactions are not supported by this wallet manager"
                    .to_string(),
            });
        }

        // Reject duplicate external ids before a nonce is reserved: the unique index
        // (idx_transaction_relayer_external_id) is the authoritative guard on save, but by
        // then the nonce manager has already been incremented and the rejected transaction
        // would strand its reserved nonce. The per-relayer queue lock is held here, so the
        // check cannot race other inserts for this relayer within this process.
        if let Some(external_id) = &transaction_to_send.external_id {
            let existing = self
                .db
                .get_transaction_by_relayer_and_external_id(relayer_id, external_id)
                .await
                .map_err(AddTransactionError::CouldNotReadExternalIdFromDb)?;

            if existing.is_some() {
                return Err(AddTransactionError::DuplicateExternalId(
                    *relayer_id,
                    external_id.clone(),
                ));
            }
        }

        // Sync nonce manager with on-chain nonce to ensure consistency
        let current_onchain_nonce = transactions_queue
            .get_nonce()
            .await
            .map_err(|e| AddTransactionError::CouldNotGetCurrentOnChainNonce(*relayer_id, e))?;

        transactions_queue.nonce_manager.sync_with_onchain_nonce(current_onchain_nonce).await;

        let mut transaction = Transaction {
            id: transaction_to_send.id,
            relayer_id: *relayer_id,
            to: transaction_to_send.to,
            from: transactions_queue.relay_address(),
            value: transaction_to_send.value,
            data: transaction_to_send.data.clone(),
            nonce: current_onchain_nonce,
            gas_limit: None,
            status: TransactionStatus::PENDING,
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
            cancelled_by_transaction_id: None,
            failed_reason: None,
            gas_price_ceiling: transaction_to_send.gas_price_ceiling,
            gas_price_ceiling_hit: false,
        };

        let (mut gas_price, blob_gas_price) = Self::compute_transaction_gas_prices(
            &transactions_queue,
            &transaction,
            &transaction_to_send.speed,
        )
        .await?;

        // Honest first-bid handling for the per-transaction gas price ceiling: freeze
        // rejects a request whose first market bid is already above the ceiling
        // (queueing it would only park a nonce we are not willing to bid), while cap
        // clamps the estimation bid exactly like the send path will.
        if transaction.apply_gas_price_ceiling(&mut gas_price)
            == GasPriceCeilingOutcome::BlockedByCeiling
        {
            return Err(AddTransactionError::GasPriceCeilingBelowMarket(
                *relayer_id,
                gas_price.max_fee,
            ));
        }
        let gas_price = gas_price;

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
                let failed_transaction =
                    Transaction { status: TransactionStatus::FAILED, ..transaction };
                self.db
                    .transaction_failed_on_send(
                        relayer_id,
                        &failed_transaction,
                        format!(
                            "Failed to send transaction as always failing on gas estimation: {err}"
                        ),
                    )
                    .await
                    .map_err(AddTransactionError::CouldNotSaveTransactionDb)?;

                self.invalidate_transaction_cache(&transaction.id).await;
                return Err(err);
            }
        };

        let assigned_nonce = transactions_queue.nonce_manager.get_and_increment().await;
        transaction.nonce = assigned_nonce;
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

        self.db.save_transaction(relayer_id, &transaction).await.map_err(|e| {
            if e.is_unique_violation_on("idx_transaction_relayer_external_id") {
                return AddTransactionError::DuplicateExternalId(
                    *relayer_id,
                    transaction.external_id.clone().unwrap_or_default(),
                );
            }

            AddTransactionError::CouldNotSaveTransactionDb(e)
        })?;

        transactions_queue.add_pending_transaction(transaction.clone()).await;
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

    /// Cancels an existing transaction.
    pub async fn cancel_transaction(
        &mut self,
        transaction: &Transaction,
    ) -> Result<CancelTransactionResult, CancelTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(&transaction.relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;
            if transactions_queue.is_paused() {
                return Err(CancelTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if let Some(mut result) =
                transactions_queue.get_editable_transaction_by_id(&transaction.id).await
            {
                let _guard = enter_critical_operation().ok_or_else(|| {
                    info!(
                        "cancel_transaction: refusing to start during shutdown for transaction {}",
                        transaction.id
                    );
                    CancelTransactionError::RelayerIsPaused(transaction.relayer_id)
                })?;

                match result.type_name {
                    EditableTransactionType::Pending => {
                        info!("cancel_transaction: converting pending transaction to no-op so its nonce is consumed");

                        transactions_queue.transaction_to_noop(&mut result.transaction);
                        result.transaction.known_transaction_hash = None;
                        result.transaction.sent_with_gas = None;
                        result.transaction.sent_with_blob_gas = None;
                        result.transaction.sent_with_max_fee_per_gas = None;
                        result.transaction.sent_with_max_priority_fee_per_gas = None;
                        result.transaction.sent_at = None;
                        result.transaction.status = TransactionStatus::PENDING;
                        transactions_queue
                            .update_pending_transaction(result.transaction.clone())
                            .await;

                        self.db
                            .transaction_update(&result.transaction)
                            .await
                            .map_err(CancelTransactionError::CouldNotUpdateTransactionDb)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let original_transaction = result.transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_cancelled(&original_transaction)
                                    .await;
                            });
                        }

                        Ok(CancelTransactionResult { success: true, cancel_transaction_id: None })
                    }
                    EditableTransactionType::Inmempool => {
                        let cancel_transaction_id = TransactionId::new();
                        let expires_at = self.expires_at(None);

                        let mut cancel_transaction = Transaction {
                            id: cancel_transaction_id,
                            relayer_id: transaction.relayer_id,
                            // Send to self (no-op)
                            to: transactions_queue.relay_address(),
                            from: transactions_queue.relay_address(),
                            value: TransactionValue::zero(),
                            data: TransactionData::empty(),
                            // Use the same nonce as the original transaction to replace it
                            nonce: result.transaction.nonce,
                            gas_limit: Some(GasLimit::new(21_000)),
                            status: TransactionStatus::PENDING,
                            blobs: None,
                            chain_id: transactions_queue.chain_id(),
                            known_transaction_hash: None,
                            queued_at: Utc::now(),
                            expires_at,
                            sent_at: None,
                            mined_at: None,
                            mined_at_block_number: None,
                            confirmed_at: None,
                            // Use highest speed for faster replacement
                            speed: TransactionSpeed::SUPER,
                            sent_with_max_priority_fee_per_gas: None,
                            sent_with_max_fee_per_gas: None,
                            is_noop: true,
                            sent_with_gas: None,
                            sent_with_blob_gas: None,
                            external_id: Some(format!("cancel_{}", transaction.id)),
                            cancelled_by_transaction_id: None,
                            failed_reason: None,
                            // A cancel must be able to outbid the original - never ceiling it
                            gas_price_ceiling: None,
                            gas_price_ceiling_hit: false,
                        };

                        info!("cancel_transaction: creating higher gas cancel transaction for inmempool tx with same nonce {:?}", cancel_transaction.nonce);

                        // For cancel transactions, we need to bump the original transaction's gas prices
                        // rather than using a fixed speed, since we're competing with the same nonce
                        let original_gas =
                            result.transaction.sent_with_gas.as_ref().ok_or_else(|| {
                                CancelTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::GasCalculationError,
                                )
                            })?;

                        // Bump original gas prices by 20% with a 1 gwei floor so tiny local
                        // raw-provider fees still produce a transaction miners prefer.
                        let bumped_max_fee =
                            bump_max_fee_for_same_nonce_competitor(original_gas.max_fee);
                        let bumped_max_priority_fee =
                            bump_max_priority_fee_for_same_nonce_competitor(
                                original_gas.max_priority_fee,
                            );

                        let gas_price = GasPriceResult {
                            max_fee: bumped_max_fee,
                            max_priority_fee: bumped_max_priority_fee,
                            min_wait_time_estimate: None,
                            max_wait_time_estimate: None,
                        };

                        // Blob gas price is not needed for cancel transactions (they're simple transfers)
                        let blob_gas_price = None;
                        // Apply gas prices to cancel transaction
                        cancel_transaction.sent_with_gas = Some(gas_price);
                        cancel_transaction.sent_with_blob_gas = blob_gas_price;

                        let transaction_sent = match transactions_queue
                            .send_transaction(&mut self.db, &mut cancel_transaction)
                            .await
                        {
                            Ok(tx_sent) => tx_sent,
                            Err(TransactionQueueSendTransactionError::TransactionSendError(
                                error,
                            )) => {
                                let error_msg = error.to_string().to_lowercase();
                                if error_msg.contains("nonce too low")
                                    || error_msg.contains("nonce is too low")
                                    || error_msg.contains("invalid nonce")
                                    || error_msg.contains("nonce has already been used")
                                    || error_msg.contains("already known")
                                {
                                    warn!("cancel_transaction: nonce synchronization issue detected for relayer {}: {}", transaction.relayer_id, error);

                                    if let Err(sync_error) = self
                                        .recover_nonce_synchronization(
                                            &transaction.relayer_id,
                                            &mut transactions_queue,
                                        )
                                        .await
                                    {
                                        error!("Failed to recover nonce synchronization for relayer {}: {}", transaction.relayer_id, sync_error);
                                        return Err(CancelTransactionError::SendTransactionError(
                                            TransactionQueueSendTransactionError::TransactionSendError(error)
                                        ));
                                    }

                                    info!("Nonce synchronization recovered for relayer {}, cancel transaction will be retried", transaction.relayer_id);
                                    return Err(
                                        CancelTransactionError::NonceSynchronizationRecovered,
                                    );
                                }
                                return Err(CancelTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::TransactionSendError(
                                        error,
                                    ),
                                ));
                            }
                            Err(e) => return Err(CancelTransactionError::SendTransactionError(e)),
                        };

                        // DON'T mark original as CANCELLED yet - that's premature!
                        // We have a race condition: both transactions will compete with same nonce
                        // The monitoring logic will determine which one wins and update statuses accordingly

                        cancel_transaction.status = TransactionStatus::INMEMPOOL;
                        cancel_transaction.known_transaction_hash = Some(transaction_sent.hash);
                        cancel_transaction.sent_at = Some(Utc::now());

                        self.db
                            .save_transaction(&transaction.relayer_id, &cancel_transaction)
                            .await
                            .map_err(CancelTransactionError::CouldNotUpdateTransactionDb)?;

                        transactions_queue
                            .add_competitor_to_inmempool_transaction(
                                &transaction.id,
                                cancel_transaction.clone(),
                                CompetitionType::Cancel,
                            )
                            .await
                            .map_err(CancelTransactionError::SendTransactionError)?;

                        // Now we can safely set the foreign key reference and update the original transaction
                        // For now, we track that a cancellation is pending by setting the cancelled_by_transaction_id
                        // but keep the original status as INMEMPOOL since it's still competing
                        result.transaction.cancelled_by_transaction_id =
                            Some(cancel_transaction_id);

                        self.db
                            .transaction_update(&result.transaction)
                            .await
                            .map_err(CancelTransactionError::CouldNotUpdateTransactionDb)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        info!("cancel_transaction: sent cancel tx {} with hash {} and nonce {:?} to replace original tx {}",
                              cancel_transaction_id, transaction_sent.hash, cancel_transaction.nonce, transaction.id);

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let original_transaction = result.transaction.clone();
                            let cancel_transaction_clone = cancel_transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_cancelled(&original_transaction)
                                    .await;
                                webhook_manager
                                    .on_transaction_sent(&cancel_transaction_clone)
                                    .await;
                            });
                        }

                        Ok(CancelTransactionResult::success(cancel_transaction_id))
                    }
                }
            } else if transactions_queue.is_transaction_mined(&transaction.id).await {
                info!(
                    "cancel_transaction: transaction {} is already mined, cannot cancel",
                    transaction.id
                );
                Ok(CancelTransactionResult::failed())
            } else {
                info!("cancel_transaction: transaction {} not found in any queue", transaction.id);
                Ok(CancelTransactionResult::failed())
            }
        } else {
            Err(CancelTransactionError::RelayerNotFound(transaction.relayer_id))
        }
    }

    /// Replaces an existing transaction with new parameters.
    pub async fn replace_transaction(
        &mut self,
        transaction: &Transaction,
        replace_with: &RelayTransactionRequest,
    ) -> Result<ReplaceTransactionResult, ReplaceTransactionError> {
        if let Some(queue_arc) = self.get_transactions_queue(&transaction.relayer_id) {
            let mut transactions_queue = queue_arc.lock().await;

            if transactions_queue.is_paused() {
                return Err(ReplaceTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if let Some(mut result) =
                transactions_queue.get_editable_transaction_by_id(&transaction.id).await
            {
                let _guard = enter_critical_operation().ok_or_else(|| {
                    info!(
                        "replace_transaction: refusing to start during shutdown for transaction {}",
                        transaction.id
                    );
                    ReplaceTransactionError::RelayerIsPaused(transaction.relayer_id)
                })?;

                match result.type_name {
                    EditableTransactionType::Pending => {
                        let original_transaction = result.transaction.clone();

                        // Swap the payload in the queue entry itself - editing only the
                        // looked-up clone would leave the ORIGINAL payload queued for
                        // broadcast while reporting success. A replacement declaring its
                        // own expiry gets a fresh deadline from now (still clamped to
                        // the global window); otherwise the slot keeps its deadline.
                        let new_expires_at = replace_with
                            .expires_in_seconds
                            .map(|_| self.expires_at(replace_with.expires_in_seconds));
                        let updated_transaction = transactions_queue
                            .replace_pending_transaction_payload(
                                &transaction.id,
                                replace_with,
                                new_expires_at,
                            )
                            .await;

                        let Some(updated_transaction) = updated_transaction else {
                            // Left the pending queue between lookup and edit
                            return Ok(ReplaceTransactionResult::failed());
                        };

                        self.db
                            .transaction_update(&updated_transaction)
                            .await
                            .map_err(ReplaceTransactionError::CouldNotUpdateTransactionInDb)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let new_transaction = updated_transaction.clone();
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

                        Ok(ReplaceTransactionResult {
                            success: true,
                            replace_transaction_id: Some(updated_transaction.id),
                            // The precomputed hash belonged to the old payload; the real
                            // hash exists once the replacement broadcasts
                            replace_transaction_hash: updated_transaction.known_transaction_hash,
                        })
                    }
                    EditableTransactionType::Inmempool => {
                        let replace_transaction_id = TransactionId::new();
                        let expires_at = self.expires_at(replace_with.expires_in_seconds);

                        let mut replace_transaction = Transaction {
                            id: replace_transaction_id,
                            relayer_id: transaction.relayer_id,
                            to: replace_with.to,
                            from: transactions_queue.relay_address(),
                            value: replace_with.value,
                            data: replace_with.data.clone(),
                            // Use the same nonce as the original transaction to replace it
                            nonce: result.transaction.nonce,
                            gas_limit: None, // Will be estimated during send_transaction
                            status: TransactionStatus::PENDING,
                            blobs: replace_with
                                .blobs
                                .as_ref()
                                .map(|blobs| {
                                    blobs
                                        .iter()
                                        .map(|blob_hex| TransactionBlob::from_hex(blob_hex))
                                        .collect::<Result<Vec<_>, _>>()
                                })
                                .transpose()
                                .map_err(|e| {
                                    ReplaceTransactionError::SendTransactionError(
                                TransactionQueueSendTransactionError::TransactionConversionError(
                                    format!("Failed to convert blob hex to TransactionBlob: {}", e)
                                )
                            )
                                })?,
                            chain_id: transactions_queue.chain_id(),
                            known_transaction_hash: None,
                            queued_at: Utc::now(),
                            expires_at,
                            sent_at: None,
                            mined_at: None,
                            mined_at_block_number: None,
                            confirmed_at: None,
                            speed: TransactionSpeed::SUPER, // Use highest speed for faster replacement
                            sent_with_max_priority_fee_per_gas: None,
                            sent_with_max_fee_per_gas: None,
                            is_noop: false,
                            sent_with_gas: None,
                            sent_with_blob_gas: None,
                            external_id: replace_with
                                .external_id
                                .clone()
                                .or_else(|| Some(format!("replace_{}", transaction.id))),
                            cancelled_by_transaction_id: None,
                            failed_reason: None,
                            gas_price_ceiling: replace_with.gas_price_ceiling,
                            gas_price_ceiling_hit: false,
                        };

                        info!("replace_transaction: creating competitive replace transaction for inmempool tx with same nonce {:?}", replace_transaction.nonce);

                        // For replace transactions, we need to bump the original transaction's gas prices and gas limit
                        // rather than using a fixed speed or estimating, since we're competing with the same nonce
                        let original_gas =
                            result.transaction.sent_with_gas.as_ref().ok_or_else(|| {
                                ReplaceTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::GasCalculationError,
                                )
                            })?;

                        // Use original gas limit + 20% to avoid "nonce too low" errors during gas estimation
                        let original_gas_limit = result.transaction.gas_limit.ok_or_else(|| {
                            ReplaceTransactionError::SendTransactionError(
                                TransactionQueueSendTransactionError::GasCalculationError,
                            )
                        })?;
                        let bumped_gas_limit = GasLimit::new(
                            original_gas_limit.into_inner() + (original_gas_limit.into_inner() / 5),
                        );
                        replace_transaction.gas_limit = Some(bumped_gas_limit);

                        // Bump original gas prices by 20% with a 1 gwei floor so tiny local
                        // raw-provider fees still produce a transaction miners prefer.
                        let bumped_max_fee =
                            bump_max_fee_for_same_nonce_competitor(original_gas.max_fee);
                        let bumped_max_priority_fee =
                            bump_max_priority_fee_for_same_nonce_competitor(
                                original_gas.max_priority_fee,
                            );

                        let gas_price = GasPriceResult {
                            max_fee: bumped_max_fee,
                            max_priority_fee: bumped_max_priority_fee,
                            min_wait_time_estimate: None,
                            max_wait_time_estimate: None,
                        };

                        let blob_gas_price = if replace_transaction.is_blob_transaction() {
                            Some(
                                transactions_queue
                                    .compute_blob_gas_price_for_transaction(
                                        &TransactionSpeed::SUPER,
                                        &None,
                                    )
                                    .await
                                    .map_err(|e| {
                                        ReplaceTransactionError::SendTransactionError(e.into())
                                    })?,
                            )
                        } else {
                            None
                        };

                        // Apply gas prices to replace transaction
                        replace_transaction.sent_with_gas = Some(gas_price);
                        replace_transaction.sent_with_blob_gas = blob_gas_price;

                        let transaction_sent = match transactions_queue
                            .send_transaction(&mut self.db, &mut replace_transaction)
                            .await
                        {
                            Ok(tx_sent) => tx_sent,
                            Err(TransactionQueueSendTransactionError::TransactionSendError(
                                error,
                            )) => {
                                let error_msg = error.to_string().to_lowercase();
                                if error_msg.contains("nonce too low")
                                    || error_msg.contains("nonce is too low")
                                    || error_msg.contains("invalid nonce")
                                    || error_msg.contains("nonce has already been used")
                                    || error_msg.contains("already known")
                                {
                                    warn!("replace_transaction: nonce synchronization issue detected for relayer {}: {}", transaction.relayer_id, error);

                                    if let Err(sync_error) = self
                                        .recover_nonce_synchronization(
                                            &transaction.relayer_id,
                                            &mut transactions_queue,
                                        )
                                        .await
                                    {
                                        error!("Failed to recover nonce synchronization for relayer {}: {}", transaction.relayer_id, sync_error);
                                        return Err(ReplaceTransactionError::SendTransactionError(
                                            TransactionQueueSendTransactionError::TransactionSendError(error)
                                        ));
                                    }

                                    info!("Nonce synchronization recovered for relayer {}, replacement transaction will be retried", transaction.relayer_id);
                                    return Err(
                                        ReplaceTransactionError::NonceSynchronizationRecovered,
                                    );
                                }
                                return Err(ReplaceTransactionError::SendTransactionError(
                                    TransactionQueueSendTransactionError::TransactionSendError(
                                        error,
                                    ),
                                ));
                            }
                            Err(e) => return Err(ReplaceTransactionError::SendTransactionError(e)),
                        };

                        replace_transaction.status = TransactionStatus::INMEMPOOL;
                        replace_transaction.known_transaction_hash = Some(transaction_sent.hash);
                        replace_transaction.sent_at = Some(Utc::now());

                        transactions_queue
                            .add_competitor_to_inmempool_transaction(
                                &transaction.id,
                                replace_transaction.clone(),
                                CompetitionType::Replace,
                            )
                            .await
                            .map_err(ReplaceTransactionError::SendTransactionError)?;

                        self.db
                            .save_transaction(&transaction.relayer_id, &replace_transaction)
                            .await
                            .map_err(ReplaceTransactionError::CouldNotUpdateTransactionInDb)?;

                        result.transaction.cancelled_by_transaction_id =
                            Some(replace_transaction_id);
                        self.db
                            .transaction_update(&result.transaction)
                            .await
                            .map_err(ReplaceTransactionError::CouldNotUpdateTransactionInDb)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        info!("replace_transaction: added competitive replace tx {} with hash {} and nonce {:?} to replace original tx {}",
                              replace_transaction_id, transaction_sent.hash, replace_transaction.nonce, transaction.id);

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let original_transaction = result.transaction.clone();
                            let replace_transaction_clone = replace_transaction.clone();
                            tokio::spawn(async move {
                                let webhook_manager = webhook_manager.lock().await;
                                webhook_manager
                                    .on_transaction_replaced(
                                        &replace_transaction_clone,
                                        &original_transaction,
                                    )
                                    .await;
                                webhook_manager
                                    .on_transaction_sent(&replace_transaction_clone)
                                    .await;
                            });
                        }

                        Ok(ReplaceTransactionResult::success(
                            replace_transaction_id,
                            transaction_sent.hash,
                        ))
                    }
                }
            } else {
                Ok(ReplaceTransactionResult::failed())
            }
        } else {
            Err(ReplaceTransactionError::TransactionNotFound(transaction.id))
        }
    }

    /// Submits a transaction at an explicit nonce already held by an in-flight
    /// transaction for the relayer, bypassing nonce allocation - the break-glass
    /// primitive for clearing a wedged nonce. A value-0 self-send with empty calldata
    /// goes through the cancel machinery (the holder resolves CANCELLED); any other
    /// payload goes through the replace machinery (the holder resolves REPLACED).
    /// Both bid a competitive fee above the holder's live bid and are signed,
    /// broadcast and tracked exactly like any other transaction.
    ///
    /// Guard rails: nonces below the relayer's current on-chain nonce are rejected
    /// (already consumed), as are nonces not held by an in-flight transaction -
    /// out-of-band broadcasts the queue is not tracking cannot be targeted. Inmempool
    /// nonces queued behind the head are rejected too: nonces resolve in order, so
    /// only the head can be wedging the relayer.
    pub async fn send_transaction_at_nonce(
        &mut self,
        relayer_id: &RelayerId,
        nonce: TransactionNonce,
        replace_with: &RelayTransactionRequest,
    ) -> Result<SendTransactionAtNonceResult, SendTransactionAtNonceError> {
        let queue_arc = self
            .get_transactions_queue(relayer_id)
            .ok_or(SendTransactionAtNonceError::RelayerNotFound(*relayer_id))?;

        // Read everything needed under the queue lock, then drop it - the cancel and
        // replace paths below re-take it themselves.
        let (relayer_address, onchain_nonce, holder) = {
            let transactions_queue = queue_arc.lock().await;

            if transactions_queue.is_paused() {
                return Err(SendTransactionAtNonceError::RelayerIsPaused(*relayer_id));
            }

            // The MINED nonce (`latest` tag): the `pending` tag counts this
            // relayer's own broadcast-but-unmined transactions, which would make
            // every INMEMPOOL holder look "already used on chain" and refuse the
            // exact replace/cancel this endpoint exists for.
            let onchain_nonce = transactions_queue.get_mined_nonce().await.map_err(|e| {
                SendTransactionAtNonceError::CouldNotGetCurrentOnChainNonce(*relayer_id, e)
            })?;

            (
                transactions_queue.relay_address(),
                onchain_nonce,
                transactions_queue.find_inflight_transaction_by_nonce(&nonce).await,
            )
        };

        let holder = validate_exact_nonce_target(relayer_id, nonce, onchain_nonce, holder)?;

        info!(
            "send_transaction_at_nonce: replacing transaction {} holding nonce {} for relayer {}",
            holder.id,
            nonce.into_inner(),
            relayer_id
        );

        if is_cancel_shaped(replace_with, &relayer_address) {
            let result = self.cancel_transaction(&holder).await?;
            if !result.success {
                return Err(SendTransactionAtNonceError::HolderNoLongerInFlight(nonce));
            }

            Ok(SendTransactionAtNonceResult {
                replaced_transaction_id: holder.id,
                transaction_id: result.cancel_transaction_id.unwrap_or(holder.id),
                hash: None,
            })
        } else {
            let result = self.replace_transaction(&holder, replace_with).await?;
            if !result.success {
                return Err(SendTransactionAtNonceError::HolderNoLongerInFlight(nonce));
            }

            Ok(SendTransactionAtNonceResult {
                replaced_transaction_id: holder.id,
                transaction_id: result.replace_transaction_id.unwrap_or(holder.id),
                hash: result.replace_transaction_hash,
            })
        }
    }

    async fn recover_nonce_synchronization(
        &mut self,
        relayer_id: &RelayerId,
        transactions_queue: &mut TransactionsQueue,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Attempting nonce recovery for relayer {}", relayer_id);

        let current_onchain_nonce = transactions_queue
            .get_nonce()
            .await
            .map_err(|e| format!("Failed to get on-chain nonce: {}", e))?;

        let current_internal_nonce = transactions_queue.nonce_manager.get_current_nonce().await;

        warn!(
            "Nonce synchronization issue detected for relayer {}: on-chain nonce is {}, internal nonce is {}",
            relayer_id, current_onchain_nonce.into_inner(), current_internal_nonce.into_inner()
        );

        transactions_queue.nonce_manager.sync_with_onchain_nonce(current_onchain_nonce).await;

        let updated_nonce = transactions_queue.nonce_manager.get_current_nonce().await;
        info!(
            "Nonce recovery completed for relayer {}: updated internal nonce to {}",
            relayer_id,
            updated_nonce.into_inner()
        );

        Ok(())
    }

    /// Closes out a pending transaction whose payload the node has permanently rejected
    /// (it would revert on-chain, its intrinsic gas is too low, or its gas limit cannot
    /// fit in a block). The caller-visible outcome is FAILED, but the reserved nonce
    /// still has to be consumed on-chain, so the queue entry is converted in place to a
    /// same-nonce no-op self-send (exactly like cancel does) instead of being dropped -
    /// dropping it would strand the nonce and wedge every transaction queued behind it.
    /// The DB row stays PENDING (with failed_reason set) so a crash before the no-op
    /// mines still rehydrates it; the no-op's receipt then resolves the status to FAILED.
    async fn close_out_pending_transaction_as_noop(
        &mut self,
        transactions_queue: &mut TransactionsQueue,
        transaction: &mut Transaction,
        reason: &str,
    ) -> Result<ProcessResult<ProcessPendingStatus>, ProcessPendingTransactionError> {
        error!(
            "process_single_pending: transaction {} permanently rejected ({}); replacing payload with a same-nonce no-op so nonce {} is still consumed",
            transaction.id,
            reason,
            transaction.nonce.into_inner()
        );

        // Snapshot BEFORE the no-op conversion so the failure webhook carries the
        // user's original payload, not the internal self-send that replaces it
        let original_payload = transaction.clone();

        transactions_queue.transaction_to_noop(transaction);
        transaction.known_transaction_hash = None;
        transaction.sent_with_gas = None;
        transaction.sent_with_max_fee_per_gas = None;
        transaction.sent_with_max_priority_fee_per_gas = None;
        transaction.sent_at = None;
        transaction.status = TransactionStatus::PENDING;
        transaction.failed_reason = Some(reason.to_string());
        transactions_queue.update_pending_transaction(transaction.clone()).await;

        // Single atomic write: transaction_update persists the no-op payload together
        // with failed_reason (and the audit row), so a crash can never separate them
        if let Err(db_error) = self.db.transaction_update(transaction).await {
            // In-memory queue is consistent; a restart re-runs this close-out.
            error!(
                "close_out_pending_transaction_as_noop: failed to persist no-op payload for transaction {}: {}",
                transaction.id, db_error
            );
        }

        self.invalidate_transaction_cache(&transaction.id).await;

        if let Some(webhook_manager) = &self.webhook_manager {
            let webhook_manager = webhook_manager.clone();
            let failed_transaction = Transaction {
                status: TransactionStatus::FAILED,
                failed_reason: Some(reason.to_string()),
                ..original_payload
            };
            tokio::spawn(async move {
                let webhook_manager = webhook_manager.lock().await;
                webhook_manager.on_transaction_failed(&failed_transaction).await;
            });
        }

        // The no-op broadcasts on the next tick at the same nonce
        Ok(ProcessResult::<ProcessPendingStatus>::other(
            ProcessPendingStatus::ClosedOutWithNoop,
            Some(&100),
        ))
    }

    /// Resolves a pending transaction whose earlier broadcast turned out to have mined
    /// (detected via a receipt for its known hash after a 'nonce too low' send error).
    /// Moves it into the inmempool queue under the mined hash so normal receipt
    /// resolution completes it - reassigning a fresh nonce here would broadcast the
    /// payload a second time.
    async fn resolve_pending_transaction_mined(
        &mut self,
        relayer_id: &RelayerId,
        relayer_address: EvmAddress,
        transactions_queue: &mut TransactionsQueue,
        transaction: &Transaction,
        mined_hash: TransactionHash,
        receipt: &AnyTransactionReceipt,
    ) -> Result<ProcessResult<ProcessPendingStatus>, ProcessPendingTransactionError> {
        // Gas fields here are bookkeeping for an already-mined broadcast - take what
        // the chain actually charged from the receipt instead of quoting the oracle
        let effective_gas_price = receipt.effective_gas_price;
        let transaction_sent = TransactionSentWithRelayer {
            id: transaction.id,
            hash: mined_hash,
            sent_with_gas: GasPriceResult {
                max_fee: MaxFee::new(effective_gas_price),
                max_priority_fee: MaxPriorityFee::new(effective_gas_price),
                min_wait_time_estimate: None,
                max_wait_time_estimate: None,
            },
            sent_with_blob_gas: None,
        };

        transactions_queue
            .move_pending_to_inmempool(transaction, &transaction_sent)
            .await
            .map_err(|e| {
                ProcessPendingTransactionError::MovePendingTransactionToInmempoolError(
                    *relayer_id,
                    relayer_address,
                    e,
                )
            })?;

        if let Err(db_error) = self
            .db
            .transaction_sent(
                &transaction_sent.id,
                &transaction_sent.hash,
                &transaction_sent.sent_with_gas,
                transaction_sent.sent_with_blob_gas.as_ref(),
                transactions_queue.is_legacy_transactions(),
            )
            .await
        {
            // The in-memory queue is consistent; a restart replays this resolution.
            error!(
                "resolve_pending_transaction_mined: failed to persist mined hash for transaction {}: {}",
                transaction.id, db_error
            );
        }

        self.invalidate_transaction_cache(&transaction.id).await;

        if let Some(webhook_manager) = &self.webhook_manager {
            let webhook_manager = webhook_manager.clone();
            let sent_transaction = Transaction {
                status: TransactionStatus::INMEMPOOL,
                known_transaction_hash: Some(transaction_sent.hash),
                sent_at: Some(Utc::now()),
                ..transaction.clone()
            };
            tokio::spawn(async move {
                let webhook_manager = webhook_manager.lock().await;
                webhook_manager.on_transaction_sent(&sent_transaction).await;
            });
        }

        Ok(ProcessResult::<ProcessPendingStatus>::other(
            ProcessPendingStatus::NonceSynchronized,
            Some(&100),
        ))
    }

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

            let relayer_address = transactions_queue.relay_address();

            if let Some(mut transaction) = transactions_queue.get_next_pending_transaction().await {
                let _guard = enter_critical_operation().ok_or_else(|| {
                    info!(
                        "process_single_pending: refusing to start during shutdown for relayer {}",
                        relayer_id
                    );
                    ProcessPendingTransactionError::RelayerTransactionsQueueNotFound(*relayer_id)
                })?;
                if TransactionsQueue::has_expired(&transaction) {
                    transactions_queue.transaction_to_noop(&mut transaction);
                }

                match transactions_queue.send_transaction(&mut self.db, &mut transaction).await {
                    Ok(transaction_sent) => {
                        transactions_queue.move_pending_to_inmempool(&transaction, &transaction_sent).await
                            .map_err(|e| ProcessPendingTransactionError::MovePendingTransactionToInmempoolError(*relayer_id, relayer_address, e))?;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let sent_transaction = Transaction {
                                status: TransactionStatus::INMEMPOOL,
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
                                    self.relayer_block_times_ms.get(relayer_id), // gas to high check back on the next block - do not do the / 10 part
                                ))
                            }
                            TransactionQueueSendTransactionError::GasPriceCeilingReached => {
                                // Freeze semantics on the first bid: never broadcast above
                                // the per-transaction ceiling. The nonce is reserved, so
                                // stay queued and re-check next block; if the market never
                                // comes back down the transaction expires through the
                                // normal no-op machinery with gas_price_ceiling_hit set.
                                self.invalidate_transaction_cache(&transaction.id).await;
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::GasPriceCeilingReached,
                                    self.relayer_block_times_ms.get(relayer_id),
                                ))
                            }
                            TransactionQueueSendTransactionError::GasCalculationError => {
                                error!(
                                    "process_single_pending: transaction {} could not calculate gas; leaving pending for retry",
                                    transaction.id
                                );
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::GasCalculationUnavailable,
                                    self.relayer_block_times_ms.get(relayer_id),
                                ))
                            }
                            TransactionQueueSendTransactionError::TransactionEstimateGasError(
                                error,
                            ) => {
                                let error_msg = error.to_string().to_lowercase();
                                match classify_send_error(&error_msg) {
                                    SendErrorClass::InsufficientFunds => {
                                        // Operator-fixable (includes geth's balance-capped
                                        // 'gas required exceeds allowance'): retry until the
                                        // relayer is topped up
                                        error!(
                                            "process_single_pending: transaction {} gas estimation hit insufficient relayer funds - retrying until topped up - {}",
                                            transaction.id, error
                                        );
                                        Ok(ProcessResult::<ProcessPendingStatus>::other(
                                            ProcessPendingStatus::SendRetrying,
                                            self.relayer_block_times_ms.get(relayer_id),
                                        ))
                                    }
                                    SendErrorClass::PermanentRejection => {
                                        // The node says this payload can never execute -
                                        // close it out as FAILED while a same-nonce no-op
                                        // consumes the reserved nonce
                                        self.close_out_pending_transaction_as_noop(
                                            &mut transactions_queue,
                                            &mut transaction,
                                            &error.to_string(),
                                        )
                                        .await
                                    }
                                    _ => {
                                        // Transient RPC issue - the nonce is already
                                        // reserved, so stay queued and retry
                                        error!(
                                            "process_single_pending: transaction {} gas estimation issue at send time - {}; leaving pending for retry",
                                            transaction.id, error
                                        );
                                        Ok(ProcessResult::<ProcessPendingStatus>::other(
                                            ProcessPendingStatus::SendRetrying,
                                            self.relayer_block_times_ms.get(relayer_id),
                                        ))
                                    }
                                }
                            }
                            TransactionQueueSendTransactionError::TransactionSendError(error) => {
                                let error_msg = error.to_string().to_lowercase();
                                let error_class = classify_send_error(&error_msg);
                                if error_class == SendErrorClass::InsufficientFunds {
                                    // Operator-fixable: the nonce was reserved at admission,
                                    // so terminally failing here would strand the nonce and
                                    // wedge every transaction queued behind it. Retry - once
                                    // the relayer is topped up the queue drains on its own.
                                    error!("process_single_pending: transaction {} cannot broadcast (insufficient relayer funds) - retrying until the relayer is topped up - error {}", transaction.id, error_msg);
                                    Ok(ProcessResult::<ProcessPendingStatus>::other(
                                        ProcessPendingStatus::SendRetrying,
                                        self.relayer_block_times_ms.get(relayer_id),
                                    ))
                                } else if error_class == SendErrorClass::PermanentRejection {
                                    // Permanently rejected payload - close it out as FAILED
                                    // while a same-nonce no-op consumes the reserved nonce
                                    self.close_out_pending_transaction_as_noop(
                                        &mut transactions_queue,
                                        &mut transaction,
                                        &error.to_string(),
                                    )
                                    .await
                                } else if error_class == SendErrorClass::AlreadyKnown {
                                    // The identical signed transaction is already live in the
                                    // node's mempool - an earlier broadcast succeeded but its
                                    // response was lost. This is success, not a nonce desync:
                                    // reassigning a fresh nonce here would execute the payload
                                    // twice. Wait a block rather than re-signing every tick -
                                    // once the live copy mines, the next attempt returns
                                    // 'nonce too low' and the receipt check below resolves it.
                                    info!(
                                        "process_single_pending: transaction {} already in mempool from a previous broadcast; waiting for it to resolve",
                                        transaction.id
                                    );
                                    Ok(ProcessResult::<ProcessPendingStatus>::other(
                                        ProcessPendingStatus::SendRetrying,
                                        self.relayer_block_times_ms.get(relayer_id),
                                    ))
                                } else if error_class == SendErrorClass::NonceConflict {
                                    warn!("process_single_pending: nonce synchronization issue detected for relayer {}: {}", relayer_id, error);

                                    // Before reassigning a fresh nonce, check whether OUR OWN
                                    // broadcast of this transaction is what consumed the nonce
                                    // (a prior lost-response send that mined). Reassigning in
                                    // that case would execute the payload a second time.
                                    if let Some(known_hash) = transaction.known_transaction_hash {
                                        match transactions_queue.get_receipt(&known_hash).await {
                                            Ok(Some(receipt)) => {
                                                info!(
                                                    "process_single_pending: transaction {} already mined as {} - handing over to receipt resolution instead of reassigning its nonce",
                                                    transaction.id, known_hash
                                                );
                                                return self
                                                    .resolve_pending_transaction_mined(
                                                        relayer_id,
                                                        relayer_address,
                                                        &mut transactions_queue,
                                                        &transaction,
                                                        known_hash,
                                                        &receipt,
                                                    )
                                                    .await;
                                            }
                                            Ok(None) => {}
                                            Err(receipt_error) => {
                                                // Fail closed: without the receipt we cannot rule
                                                // out that our own broadcast consumed the nonce,
                                                // and reassigning would risk executing the payload
                                                // twice. Retry the whole check next tick.
                                                warn!(
                                                    "process_single_pending: could not check receipt for transaction {} ({}) - retrying before touching its nonce",
                                                    transaction.id, receipt_error
                                                );
                                                return Ok(
                                                    ProcessResult::<ProcessPendingStatus>::other(
                                                        ProcessPendingStatus::SendRetrying,
                                                        Some(&100),
                                                    ),
                                                );
                                            }
                                        }
                                    }

                                    if let Err(sync_error) = self
                                        .recover_nonce_synchronization(
                                            relayer_id,
                                            &mut transactions_queue,
                                        )
                                        .await
                                    {
                                        error!("Failed to recover nonce synchronization for relayer {}: {}", relayer_id, sync_error);
                                        return Err(ProcessPendingTransactionError::SendTransactionError(
                                            *relayer_id,
                                            relayer_address,
                                            TransactionQueueSendTransactionError::TransactionSendError(error),
                                        ));
                                    }

                                    let new_nonce =
                                        transactions_queue.nonce_manager.get_and_increment().await;
                                    transaction.nonce = new_nonce;

                                    transactions_queue
                                        .update_pending_transaction_nonce(
                                            &transaction.id,
                                            new_nonce,
                                        )
                                        .await;

                                    if let Err(db_error) = self
                                        .db
                                        .transaction_update_nonce(&transaction.id, &new_nonce)
                                        .await
                                    {
                                        error!("Failed to persist nonce update to database for transaction {}: {}", transaction.id, db_error);
                                    }

                                    info!("Nonce synchronization recovered for relayer {}, updated pending transaction nonce to {} in queue and database", relayer_id, new_nonce.into_inner());

                                    Ok(ProcessResult::<ProcessPendingStatus>::other(
                                        ProcessPendingStatus::NonceSynchronized,
                                        Some(&100),
                                    ))
                                } else {
                                    // For other send errors (RPC down, etc), keep as temp issue
                                    Err(ProcessPendingTransactionError::SendTransactionError(
                                        *relayer_id,
                                        relayer_address,
                                        TransactionQueueSendTransactionError::TransactionSendError(
                                            error,
                                        ),
                                    ))
                                }
                            }
                            TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(
                                error,
                            ) => {
                                // just keep the transaction in pending state as could be a bad
                                // db connection or temp
                                // outage
                                Err(ProcessPendingTransactionError::SendTransactionError(
                                    *relayer_id,
                                    relayer_address,
                                    TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(error),
                                ))
                            }
                            TransactionQueueSendTransactionError::SendTransactionGasPriceError(
                                error,
                            ) => {
                                error!(
                                    "process_single_pending: transaction {} could not calculate gas price - {}; leaving pending for retry",
                                    transaction.id, error
                                );
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::GasCalculationUnavailable,
                                    self.relayer_block_times_ms.get(relayer_id),
                                ))
                            }
                            TransactionQueueSendTransactionError::TransactionConversionError(
                                error,
                            ) => {
                                // Dominated by transient causes: remote signer outages are
                                // wrapped into conversion errors, and for Safe relayers the
                                // Safe wrapping/signing only happens at send time. Keep the
                                // nonce-holding transaction queued and retry; a genuinely
                                // unconvertible payload is closed out via cancel (same-nonce
                                // no-op).
                                error!(
                                    "process_single_pending: transaction {} conversion issue at send time - {}; leaving pending for retry",
                                    transaction.id, error
                                );
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::SendRetrying,
                                    self.relayer_block_times_ms.get(relayer_id),
                                ))
                            }
                            TransactionQueueSendTransactionError::SafeProxyError(error) => {
                                // Reading the Safe contract nonce is a live eth_call on every
                                // send - a transient RPC failure here must not drop the
                                // nonce-holding transaction.
                                error!(
                                    "process_single_pending: transaction {} safe proxy issue at send time - {}; leaving pending for retry",
                                    transaction.id, error
                                );
                                Ok(ProcessResult::<ProcessPendingStatus>::other(
                                    ProcessPendingStatus::SendRetrying,
                                    self.relayer_block_times_ms.get(relayer_id),
                                ))
                            }
                            TransactionQueueSendTransactionError::NoTransactionInQueue => {
                                // This shouldn't happen in normal flow, but if it does, keep the transaction pending
                                Err(ProcessPendingTransactionError::SendTransactionError(
                                    *relayer_id,
                                    relayer_address,
                                    TransactionQueueSendTransactionError::NoTransactionInQueue,
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

            let relayer_address = transactions_queue.relay_address();

            if let Some(mut transaction) = transactions_queue.get_next_inmempool_transaction().await
            {
                let _guard = enter_critical_operation().ok_or_else(|| {
                    info!(
                        "process_single_inmempool: refusing to start during shutdown for relayer {}",
                        relayer_id
                    );
                    ProcessInmempoolTransactionError::RelayerTransactionsQueueNotFound(*relayer_id)
                })?;
                if let Some(known_transaction_hash) = transaction.known_transaction_hash {
                    match transactions_queue.get_receipt(&known_transaction_hash).await {
                        Ok(Some(receipt)) => {
                            let competition_result = transactions_queue
                                .move_inmempool_to_mining(&transaction.id, &receipt)
                                .await.map_err(|e| ProcessInmempoolTransactionError::MoveInmempoolTransactionToMinedError(*relayer_id, relayer_address, e))?;

                            // Save the winning transaction to database
                            match competition_result.winner_status {
                                TransactionStatus::MINED => {
                                    self.db
                                        .transaction_mined(&competition_result.winner, &receipt)
                                        .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, relayer_address, competition_result.winner.clone(), TransactionStatus::MINED, e))?;
                                    self.invalidate_transaction_cache(
                                        &competition_result.winner.id,
                                    )
                                    .await;

                                    // Save the loser transaction to database if there was competition
                                    if let Some(loser) = &competition_result.loser {
                                        self.db
                                            .transaction_update(loser)
                                            .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, relayer_address, loser.clone(), loser.status, e))?;
                                        self.invalidate_transaction_cache(&loser.id).await;

                                        info!("Updated loser transaction {} with status {:?} in database", loser.id, loser.status);
                                    }

                                    if let Some(webhook_manager) = &self.webhook_manager {
                                        let webhook_manager = webhook_manager.clone();
                                        let mined_transaction = competition_result.winner.clone();
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
                                TransactionStatus::EXPIRED => {
                                    self.db.transaction_expired(&competition_result.winner.id).await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, relayer_address, competition_result.winner.clone(), TransactionStatus::EXPIRED, e))?;
                                    self.invalidate_transaction_cache(
                                        &competition_result.winner.id,
                                    )
                                    .await;

                                    if let Some(webhook_manager) = &self.webhook_manager {
                                        let webhook_manager = webhook_manager.clone();
                                        let expired_transaction = competition_result.winner.clone();
                                        tokio::spawn(async move {
                                            let webhook_manager = webhook_manager.lock().await;
                                            webhook_manager
                                                .on_transaction_expired(&expired_transaction)
                                                .await;
                                        });
                                    }
                                }
                                TransactionStatus::FAILED => {
                                    // A close-out no-op carries the node's original
                                    // rejection reason - preserve it instead of the
                                    // generic on-chain-failure message
                                    let was_closed_out =
                                        competition_result.winner.failed_reason.is_some();
                                    let failed_reason = competition_result
                                        .winner
                                        .failed_reason
                                        .clone()
                                        .unwrap_or_else(|| "Failed onchain".to_string());
                                    self.db
                                        .update_transaction_failed(&competition_result.winner.id, &failed_reason)
                                        .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, relayer_address, competition_result.winner.clone(), TransactionStatus::FAILED, e))?;
                                    self.invalidate_transaction_cache(
                                        &competition_result.winner.id,
                                    )
                                    .await;

                                    // Close-outs already fired their failure webhook with
                                    // the original payload when the rejection happened
                                    if !was_closed_out {
                                        if let Some(webhook_manager) = &self.webhook_manager {
                                            let webhook_manager = webhook_manager.clone();
                                            let failed_transaction =
                                                competition_result.winner.clone();
                                            tokio::spawn(async move {
                                                let webhook_manager = webhook_manager.lock().await;
                                                webhook_manager
                                                    .on_transaction_failed(&failed_transaction)
                                                    .await;
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }

                            Ok(ProcessResult::<ProcessInmempoolStatus>::success())
                        }
                        Ok(None) => {
                            if let Some(sent_at) = transaction.sent_at {
                                let elapsed = Utc::now() - sent_at;

                                let at_max_gas_cap =
                                    if let Some(ref sent_gas) = transaction.sent_with_gas {
                                        transactions_queue.is_at_max_gas_price_cap(sent_gas).await
                                    } else {
                                        false
                                    };

                                let at_max_blob_gas_cap = if let Some(ref sent_blob_gas) =
                                    transaction.sent_with_blob_gas
                                {
                                    transactions_queue
                                        .is_at_max_blob_gas_price_cap(sent_blob_gas)
                                        .await
                                } else {
                                    false
                                };

                                if at_max_gas_cap || at_max_blob_gas_cap {
                                    info!(
                                        "Transaction {} has reached maximum gas price cap (gas: {}, blob: {}), skipping gas bump for relayer: {}",
                                        transaction.id, at_max_gas_cap, at_max_blob_gas_cap, transactions_queue.relayer_name()
                                    );
                                    return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                        ProcessInmempoolStatus::StillInmempool,
                                        self.relayer_block_times_ms
                                            .get(relayer_id)
                                            .map(|&block_time| block_time / 10)
                                            .as_ref(),
                                    ));
                                }

                                if transactions_queue.should_bump_gas(
                                    elapsed.num_milliseconds() as u64,
                                    &transaction.speed,
                                ) {
                                    let was_noop = transaction.is_noop;
                                    let transaction_sent = match transactions_queue
                                        .send_transaction(&mut self.db, &mut transaction)
                                        .await
                                    {
                                        Ok(tx_sent) => tx_sent,
                                        Err(TransactionQueueSendTransactionError::GasPriceCeilingReached) => {
                                            // The per-transaction ceiling bound this bump
                                            // (freeze keeps the last compliant bid live; cap is
                                            // already bidding exactly the ceiling). Leave the
                                            // live bid to race until it mines or expires.
                                            info!(
                                                "Transaction {} gas price ceiling reached during bump for relayer: {} - keeping current bid",
                                                transaction.id, transactions_queue.relayer_name()
                                            );
                                            self.invalidate_transaction_cache(&transaction.id).await;
                                            return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                                ProcessInmempoolStatus::StillInmempool,
                                                self.relayer_block_times_ms.get(relayer_id),
                                            ));
                                        }
                                        Err(TransactionQueueSendTransactionError::TransactionSendError(error)) => {
                                            let error_msg = error.to_string().to_lowercase();
                                            let error_class = classify_send_error(&error_msg);
                                            if error_class == SendErrorClass::AlreadyKnown {
                                                // The bumped payload is already in the node's
                                                // pool (a previous broadcast succeeded but the
                                                // response was lost). Keep polling the receipt -
                                                // never reassign the nonce of a live transaction,
                                                // and wait a block rather than re-signing per tick.
                                                info!("process_single_inmempool: gas bump for transaction {} already in mempool; continuing receipt polling", transaction.id);
                                                return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                                    ProcessInmempoolStatus::StillInmempool,
                                                    self.relayer_block_times_ms.get(relayer_id),
                                                ));
                                            }
                                            if error_class == SendErrorClass::NonceConflict {
                                                warn!("process_single_inmempool: nonce synchronization issue detected for relayer {} during gas bump: {}", relayer_id, error);

                                                // 'nonce too low' during a bump most commonly
                                                // means this very transaction mined between the
                                                // receipt poll and the bump send. Check the
                                                // receipt before assuming the nonce was consumed
                                                // externally - reassigning a mined transaction a
                                                // fresh nonce would re-execute its payload and
                                                // strand the newly reserved nonce forever.
                                                if let Some(known_hash) = transaction.known_transaction_hash {
                                                    match transactions_queue.get_receipt(&known_hash).await {
                                                        Ok(Some(_receipt)) => {
                                                            info!("process_single_inmempool: transaction {} mined as {} during bump race; letting receipt resolution complete it", transaction.id, known_hash);
                                                            return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                                                ProcessInmempoolStatus::StillInmempool,
                                                                Some(&100),
                                                            ));
                                                        }
                                                        Ok(None) => {}
                                                        Err(receipt_error) => {
                                                            // Fail closed: without the receipt we cannot rule out that
                                                            // this transaction mined, and reassigning its nonce would
                                                            // risk executing the payload twice. Retry next tick.
                                                            warn!("process_single_inmempool: could not check receipt for transaction {} ({}) - retrying before touching its nonce", transaction.id, receipt_error);
                                                            return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                                                ProcessInmempoolStatus::StillInmempool,
                                                                Some(&100),
                                                            ));
                                                        }
                                                    }
                                                }

                                                if let Err(sync_error) = self.recover_nonce_synchronization(relayer_id, &mut transactions_queue).await {
                                                    error!("Failed to recover nonce synchronization for relayer {}: {}", relayer_id, sync_error);
                                                    return Err(ProcessInmempoolTransactionError::SendTransactionError(
                                                        *relayer_id,
                                                        relayer_address,
                                                        TransactionQueueSendTransactionError::TransactionSendError(error)
                                                    ));
                                                }

                                                let new_nonce = transactions_queue.nonce_manager.get_and_increment().await;
                                                transaction.nonce = new_nonce;

                                                transactions_queue.update_inmempool_transaction_nonce(&transaction.id, new_nonce).await;

                                                if let Err(db_error) = self.db.transaction_update_nonce(&transaction.id, &new_nonce).await {
                                                    error!("Failed to persist nonce update to database for transaction {}: {}", transaction.id, db_error);
                                                }

                                                info!("Nonce synchronization recovered for relayer {}, updated gas bump transaction nonce {} in queue and database", relayer_id, new_nonce.into_inner());

                                                return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                                    ProcessInmempoolStatus::NonceSynchronized,
                                                    Some(&100),
                                                ));
                                            }
                                            return Err(ProcessInmempoolTransactionError::SendTransactionError(
                                                *relayer_id,
                                                relayer_address,
                                                TransactionQueueSendTransactionError::TransactionSendError(error)
                                            ));
                                        }
                                        Err(e) => return Err(ProcessInmempoolTransactionError::SendTransactionError(*relayer_id, relayer_address, e)),
                                    };

                                    // Update the actual transaction in the inmempool queue
                                    transactions_queue
                                        .update_inmempool_transaction_gas(&transaction_sent)
                                        .await;

                                    // If the transaction expired mid-send and was replaced with a
                                    // no-op, the inmempool entry must reflect that so it resolves
                                    // to EXPIRED (not MINED) once the no-op lands
                                    if !was_noop && transaction.is_noop {
                                        transactions_queue
                                            .update_inmempool_transaction_noop(
                                                &transaction.id,
                                                &transaction_sent,
                                            )
                                            .await;
                                    }

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

                                    if let Err(db_error) = self
                                        .db
                                        .transaction_sent(
                                            &transaction_sent.id,
                                            &transaction_sent.hash,
                                            &transaction_sent.sent_with_gas,
                                            transaction_sent.sent_with_blob_gas.as_ref(),
                                            transactions_queue.is_legacy_transactions(),
                                        )
                                        .await
                                    {
                                        error!("Failed to persist gas bump to database for transaction {}: {}", transaction.id, db_error);
                                    }

                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                        ProcessInmempoolStatus::GasIncreased,
                                        Default::default(),
                                    ));
                                }
                            }

                            Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                ProcessInmempoolStatus::StillInmempool,
                                self.relayer_block_times_ms
                                    .get(relayer_id)
                                    .map(|&block_time| block_time / 10)
                                    .as_ref(),
                            ))
                        }
                        Err(e) => {
                            Err(ProcessInmempoolTransactionError::CouldNotGetTransactionReceipt(
                                *relayer_id,
                                relayer_address,
                                transaction.clone(),
                                e,
                            ))
                        }
                    }
                } else {
                    Err(ProcessInmempoolTransactionError::UnknownTransactionHash(
                        *relayer_id,
                        relayer_address,
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

            let relayer_address = transactions_queue.relay_address();

            if let Some(transaction) = transactions_queue.get_next_mined_transaction().await {
                let _guard = enter_critical_operation().ok_or_else(|| {
                    info!(
                        "process_single_mined: refusing to start during shutdown for relayer {}",
                        relayer_id
                    );
                    ProcessMinedTransactionError::RelayerTransactionsQueueNotFound(*relayer_id)
                })?;

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
                                        relayer_address,
                                        transaction.clone(),
                                        e,
                                    )
                                })?
                                .ok_or(
                                    ProcessMinedTransactionError::CouldNotGetTransactionReceipt(
                                        *relayer_id,
                                        relayer_address,
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
                                    relayer_address,
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
                                relayer_address,
                                transaction.clone(),
                                e,
                            )
                        })?;

                        transactions_queue.move_mining_to_confirmed(&transaction.id).await;

                        self.invalidate_transaction_cache(&transaction.id).await;

                        if let Some(webhook_manager) = &self.webhook_manager {
                            let webhook_manager = webhook_manager.clone();
                            let confirmed_transaction = Transaction {
                                status: TransactionStatus::CONFIRMED,
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
                    Err(ProcessMinedTransactionError::NoMinedAt(
                        *relayer_id,
                        relayer_address,
                        transaction.clone(),
                    ))
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

#[cfg(test)]
mod tests {
    use super::SendTransactionAtNonceError;
    use super::{
        is_cancel_shaped, resolve_expiry_duration, validate_exact_nonce_target, InflightNonceHolder,
    };
    use crate::relayer::RelayerId;
    use crate::shared::common_types::EvmAddress;
    use crate::transaction::api::RelayTransactionRequest;
    use crate::transaction::types::{
        test_fixtures::test_transaction, TransactionData, TransactionNonce, TransactionStatus,
        TransactionValue,
    };

    fn relayer_address() -> EvmAddress {
        EvmAddress::from(alloy::primitives::Address::repeat_byte(0x11))
    }

    fn request(to: EvmAddress, value: u128, data: TransactionData) -> RelayTransactionRequest {
        RelayTransactionRequest {
            to,
            value: TransactionValue::new(alloy::primitives::U256::from(value)),
            data,
            speed: None,
            external_id: None,
            blobs: None,
            gas_price_ceiling: None,
            expires_in_seconds: None,
        }
    }

    #[test]
    fn a_value_zero_self_send_with_no_calldata_is_cancel_shaped() {
        let address = relayer_address();
        assert!(is_cancel_shaped(&request(address, 0, TransactionData::empty()), &address));

        let mut with_empty_blobs = request(address, 0, TransactionData::empty());
        with_empty_blobs.blobs = Some(vec![]);
        assert!(is_cancel_shaped(&with_empty_blobs, &address));
    }

    #[test]
    fn other_payloads_are_not_cancel_shaped() {
        let address = relayer_address();
        let other = EvmAddress::from(alloy::primitives::Address::repeat_byte(0x22));

        // Not a self-send
        assert!(!is_cancel_shaped(&request(other, 0, TransactionData::empty()), &address));
        // Carries value
        assert!(!is_cancel_shaped(&request(address, 1, TransactionData::empty()), &address));
        // Carries calldata
        assert!(!is_cancel_shaped(
            &request(address, 0, TransactionData::new(vec![0x01].into())),
            &address
        ));
        // Carries blobs
        let mut with_blobs = request(address, 0, TransactionData::empty());
        with_blobs.blobs = Some(vec!["0x01".to_string()]);
        assert!(!is_cancel_shaped(&with_blobs, &address));
    }

    #[test]
    fn exact_nonce_send_rejects_nonces_already_used_on_chain() {
        let relayer_id = RelayerId::new();
        let holder = test_transaction(4, TransactionStatus::INMEMPOOL);

        let result = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(4),
            TransactionNonce::new(5),
            InflightNonceHolder::InmempoolHead(holder),
        );

        assert!(matches!(
            result,
            Err(SendTransactionAtNonceError::NonceAlreadyUsedOnchain(_, _, _))
        ));
    }

    // Regression test: the guard must compare against the MINED (latest-tag) nonce.
    // The pending tag counts the relayer's own broadcast-but-unmined transactions -
    // it reads N+1 for an inmempool holder at nonce N, which made every exact-nonce
    // cancel/replace of a stuck broadcast trip NonceAlreadyUsedOnchain.
    #[test]
    fn exact_nonce_send_allows_replacing_a_broadcast_but_unmined_holder() {
        let relayer_id = RelayerId::new();

        // Broadcast but unmined: the mined nonce is still 7 (the pending-tag nonce
        // would already read 8 and wrongly reject the holder)
        let holder = test_transaction(7, TransactionStatus::INMEMPOOL);
        let resolved = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(7),
            TransactionNonce::new(7),
            InflightNonceHolder::InmempoolHead(holder.clone()),
        )
        .expect("a broadcast-but-unmined holder must stay replaceable at its nonce");
        assert_eq!(resolved.id, holder.id);

        // Genuinely mined past the nonce: the mined nonce moved to 8, the slot is gone
        let result = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(7),
            TransactionNonce::new(8),
            InflightNonceHolder::InmempoolHead(test_transaction(7, TransactionStatus::INMEMPOOL)),
        );
        assert!(matches!(
            result,
            Err(SendTransactionAtNonceError::NonceAlreadyUsedOnchain(_, _, _))
        ));
    }

    #[test]
    fn exact_nonce_send_rejects_nonces_not_held_by_an_inflight_transaction() {
        let relayer_id = RelayerId::new();

        let result = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(9),
            TransactionNonce::new(5),
            InflightNonceHolder::NotFound,
        );

        assert!(matches!(result, Err(SendTransactionAtNonceError::NonceNotPending(_, _))));
    }

    #[test]
    fn exact_nonce_send_rejects_inmempool_nonces_behind_the_head() {
        let relayer_id = RelayerId::new();
        let holder = test_transaction(6, TransactionStatus::INMEMPOOL);

        let result = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(6),
            TransactionNonce::new(5),
            InflightNonceHolder::InmempoolBehindHead(holder),
        );

        assert!(matches!(result, Err(SendTransactionAtNonceError::NonceBehindInmempoolHead(_, _))));
    }

    #[test]
    fn exact_nonce_send_accepts_pending_and_inmempool_head_holders() {
        let relayer_id = RelayerId::new();

        let pending = test_transaction(5, TransactionStatus::PENDING);
        let resolved = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(5),
            TransactionNonce::new(5),
            InflightNonceHolder::Pending(pending.clone()),
        )
        .expect("a pending holder at the current nonce should be replaceable");
        assert_eq!(resolved.id, pending.id);

        let head = test_transaction(5, TransactionStatus::INMEMPOOL);
        let resolved = validate_exact_nonce_target(
            &relayer_id,
            TransactionNonce::new(5),
            TransactionNonce::new(5),
            InflightNonceHolder::InmempoolHead(head.clone()),
        )
        .expect("the inmempool head should be replaceable");
        assert_eq!(resolved.id, head.id);
    }

    #[test]
    fn per_transaction_expiry_overrides_the_global_default() {
        let global = chrono::Duration::hours(12);

        let resolved = resolve_expiry_duration(global, Some(300));

        assert_eq!(resolved, chrono::Duration::seconds(300));
    }

    #[test]
    fn per_transaction_expiry_longer_than_global_clamps_to_global() {
        let global = chrono::Duration::hours(12);

        let thirteen_hours = 13 * 60 * 60;
        assert_eq!(resolve_expiry_duration(global, Some(thirteen_hours)), global);

        // Equal to the global window is the global window
        let twelve_hours = 12 * 60 * 60;
        assert_eq!(resolve_expiry_duration(global, Some(twelve_hours)), global);

        // Values chrono cannot represent clamp to the operator ceiling too
        assert_eq!(resolve_expiry_duration(global, Some(u64::MAX)), global);
    }

    #[test]
    fn absent_per_transaction_expiry_keeps_the_global_window() {
        let global = chrono::Duration::hours(12);

        assert_eq!(resolve_expiry_duration(global, None), global);
    }
}
