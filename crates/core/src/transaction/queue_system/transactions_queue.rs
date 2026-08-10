use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use super::types::{
    CompetitionResolutionResult, CompetitionType, CompetitiveTransaction, EditableTransaction,
    MoveInmempoolTransactionToMinedError, MovePendingTransactionToInmempoolError,
    SendTransactionGasPriceError, TransactionQueueSendTransactionError, TransactionSentWithRelayer,
    TransactionsQueueSetup,
};
use crate::transaction::api::RelayTransactionRequest;
use crate::transaction::types::{
    GasPriceCeilingOutcome, TransactionBlob, TransactionNonce, TransactionValue,
};
use crate::{
    gas::{
        BlobGasOracleCache, BlobGasPriceResult, GasLimit, GasOracleCache, GasPrice, GasPriceResult,
        MaxFee, MaxPriorityFee, BLOB_GAS_PER_BLOB,
    },
    network::ChainId,
    postgres::PostgresClient,
    provider::{EvmProvider, SendTransactionError},
    relayer::{Relayer, RelayerId},
    safe_proxy::SafeProxyManager,
    shared::common_types::EvmAddress,
    transaction::types::TransactionData,
    transaction::{
        nonce_manager::NonceManager,
        types::{Transaction, TransactionHash, TransactionId, TransactionSpeed, TransactionStatus},
    },
    yaml::GasBumpBlockConfig,
    WalletError,
};
use alloy::network::{AnyTransactionReceipt, ReceiptResponse};
use alloy::{
    consensus::{SignableTransaction, TypedTransaction},
    hex,
    primitives::Signature,
    transports::{RpcError, TransportErrorKind},
};
use chrono::Utc;
use tokio::sync::Mutex;
use tracing::error;
use tracing::info;

/// How the queue must react to a node's send/estimate error. Classification is
/// centralised here so the pending loop, the gas-bump loop, and broadcast-hash
/// recording can never drift apart on the same error string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendErrorClass {
    /// Operator-fixable: the relayer cannot pay for the transaction right now.
    /// Retry in place until it is topped up - the reserved nonce must not be dropped.
    InsufficientFunds,
    /// The node says this payload can never execute (would revert, intrinsic gas too
    /// low, over the block gas cap). Close out: mark FAILED and consume the reserved
    /// nonce with a same-nonce no-op.
    PermanentRejection,
    /// The identical signed payload is already live in the node's mempool - an earlier
    /// broadcast succeeded. Never reassign this nonce; keep polling until it resolves.
    AlreadyKnown,
    /// The nonce was consumed by some broadcast. Check our own receipts before
    /// concluding it was external and reassigning.
    NonceConflict,
    /// A same-nonce replacement was rejected for insufficient fee bump. The existing
    /// broadcast is still live; retry with backoff.
    Underpriced,
    /// Transport failures and unrecognised wording - the outcome is unknown, retry.
    Transient,
}

/// Classifies a lowercased node error message. Match order matters: permanent
/// rejections are checked first because a revert reason can quote any wording -
/// 'execution reverted: ERC20: transfer amount exceeds balance' must not be read
/// as relayer-insufficient-funds, and 'execution reverted: invalid nonce' (common
/// in forwarder/meta-tx contracts) must not trigger nonce resynchronisation -
/// while genuine node-level nonce/mempool/funds errors never contain 'execution
/// reverted'. geth's 'gas required exceeds allowance' is a balance-capped
/// estimation (operator-fixable), not a payload defect.
pub fn classify_send_error(error_msg: &str) -> SendErrorClass {
    if error_msg.contains("execution reverted")
        || error_msg.contains("invalid opcode")
        || error_msg.contains("intrinsic gas too low")
        || error_msg.contains("exceeds block gas limit")
        || error_msg.contains("oversized data")
        || error_msg.contains("max initcode size exceeded")
        || error_msg.contains("maxtxsizeexceeded")
    {
        return SendErrorClass::PermanentRejection;
    }
    if error_msg.contains("already known")
        || error_msg.contains("alreadyknown")
        || error_msg.contains("known transaction")
        || error_msg.contains("already imported")
    {
        return SendErrorClass::AlreadyKnown;
    }
    if error_msg.contains("nonce too low")
        || error_msg.contains("nonce is too low")
        || error_msg.contains("invalid nonce")
        || error_msg.contains("nonce has already been used")
        || error_msg.contains("oldnonce")
    {
        return SendErrorClass::NonceConflict;
    }
    if error_msg.contains("insufficient funds")
        || error_msg.contains("insufficientfunds")
        || error_msg.contains("gas required exceeds allowance")
        || error_msg.contains("overshot")
    {
        return SendErrorClass::InsufficientFunds;
    }
    if error_msg.contains("underpriced") || error_msg.contains("feetoolow") {
        return SendErrorClass::Underpriced;
    }
    SendErrorClass::Transient
}

fn bump_u128_by_at_least_one(value: u128) -> u128 {
    value + std::cmp::max(value / 20, 1)
}

fn bump_max_fee_by_at_least_one(max_fee: MaxFee) -> MaxFee {
    MaxFee::new(bump_u128_by_at_least_one(max_fee.into_u128()))
}

fn bump_max_priority_fee_by_at_least_one(max_priority_fee: MaxPriorityFee) -> MaxPriorityFee {
    MaxPriorityFee::new(bump_u128_by_at_least_one(max_priority_fee.into_u128()))
}

/// Where (if anywhere) a nonce currently sits in a relayer's in-flight queues.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum InflightNonceHolder {
    /// Held by a queued transaction whose nonce is reserved but not broadcast yet.
    Pending(Transaction),
    /// Held by the head of the inmempool queue - the only broadcast transaction a
    /// same-nonce competitor can be attached to.
    InmempoolHead(Transaction),
    /// Held by a broadcast transaction queued behind the inmempool head; the head
    /// nonce has to resolve first.
    InmempoolBehindHead(Transaction),
    /// Not held by any in-flight transaction for this relayer.
    NotFound,
}

/// Pure snapshot of the live queues - pending first (queue order), then inmempool
/// originals with their same-nonce competitors. Kept free of `TransactionsQueue`
/// so the listing shape stays unit-testable without a provider.
fn snapshot_inflight_transactions(
    pending: &VecDeque<Transaction>,
    inmempool: &VecDeque<CompetitiveTransaction>,
) -> Vec<Transaction> {
    let mut transactions: Vec<Transaction> = pending.iter().cloned().collect();

    for comp_tx in inmempool {
        transactions.push(comp_tx.original.clone());
        if let Some((competitor, _)) = &comp_tx.competitive {
            transactions.push(competitor.clone());
        }
    }

    transactions
}

/// Rewrites a transaction's payload with the replacement request. The gas limit is
/// cleared so the send path re-estimates for the new payload, and `is_noop` is
/// recomputed from the new destination. The replacement carries its own gas price
/// ceiling terms, so a previous ceiling hit no longer applies.
fn apply_replacement_payload(
    transaction: &mut Transaction,
    replace_with: &RelayTransactionRequest,
) {
    transaction.to = replace_with.to;
    transaction.data = replace_with.data.clone();
    transaction.value = replace_with.value;
    transaction.is_noop = transaction.from == transaction.to;

    if let Some(ref blob_strings) = replace_with.blobs {
        transaction.blobs = Some(
            blob_strings
                .iter()
                .map(|blob_hex| TransactionBlob::from_hex(blob_hex))
                .collect::<Result<Vec<_>, _>>()
                .expect("Failed to convert blob hex strings to TransactionBlob"),
        );
    } else {
        transaction.blobs = None;
    }
    transaction.gas_limit = None;
    transaction.external_id = replace_with.external_id.clone();
    transaction.gas_price_ceiling = replace_with.gas_price_ceiling;
    transaction.gas_price_ceiling_hit = false;
}

/// Swaps a PENDING transaction's payload in the queue itself, returning the updated
/// transaction for persistence, or `None` when the id is no longer pending. The
/// precomputed hash is cleared - it belongs to the old payload; the send path
/// computes the real hash when the replacement broadcasts. Kept free of
/// `TransactionsQueue` so the in-place swap stays unit-testable without a provider.
fn replace_pending_transaction_payload_in_queue(
    pending: &mut VecDeque<Transaction>,
    transaction_id: &TransactionId,
    replace_with: &RelayTransactionRequest,
    new_expires_at: Option<chrono::DateTime<Utc>>,
) -> Option<Transaction> {
    let transaction = pending.iter_mut().find(|tx| tx.id == *transaction_id)?;

    apply_replacement_payload(transaction, replace_with);
    transaction.known_transaction_hash = None;
    // Only a replacement that declares its own expiry moves the deadline - otherwise
    // the queued slot keeps the deadline the original request was admitted with
    if let Some(expires_at) = new_expires_at {
        transaction.expires_at = expires_at;
    }

    Some(transaction.clone())
}

/// Converts a transaction into the same-nonce close-out no-op: a value-0 empty-data
/// self-send that consumes the reserved nonce (used when a transaction expires or its
/// payload is permanently rejected). The close-out must always be broadcastable, so
/// the original payload's gas price ceiling no longer applies - the reserved nonce is
/// consumed at market price - while `gas_price_ceiling_hit` is kept so callers can
/// still see the ceiling bound the original payload.
fn convert_transaction_to_noop(transaction: &mut Transaction, relayer_address: EvmAddress) {
    transaction.to = relayer_address;
    transaction.value = TransactionValue::zero();
    transaction.data = TransactionData::empty();
    transaction.blobs = None;
    transaction.gas_limit = Some(GasLimit::new(21_000));
    transaction.is_noop = true;
    transaction.speed = TransactionSpeed::FAST;
    transaction.sent_with_blob_gas = None;
    transaction.gas_price_ceiling = None;
}

/// Pure lookup of which in-flight transaction holds a nonce. Inmempool matches are
/// resolved against the ORIGINAL transaction (competitors share its nonce) since the
/// cancel/replace machinery is keyed by the original's id.
fn find_inflight_transaction_by_nonce(
    pending: &VecDeque<Transaction>,
    inmempool: &VecDeque<CompetitiveTransaction>,
    nonce: &TransactionNonce,
) -> InflightNonceHolder {
    if let Some(transaction) = pending.iter().find(|tx| tx.nonce == *nonce) {
        return InflightNonceHolder::Pending(transaction.clone());
    }

    for (position, comp_tx) in inmempool.iter().enumerate() {
        if comp_tx.original.nonce == *nonce {
            return if position == 0 {
                InflightNonceHolder::InmempoolHead(comp_tx.original.clone())
            } else {
                InflightNonceHolder::InmempoolBehindHead(comp_tx.original.clone())
            };
        }
    }

    InflightNonceHolder::NotFound
}

pub struct TransactionsQueue {
    pending_transactions: Mutex<VecDeque<Transaction>>,
    inmempool_transactions: Mutex<VecDeque<CompetitiveTransaction>>,
    mined_transactions: Mutex<HashMap<TransactionId, Transaction>>,
    evm_provider: EvmProvider,
    relayer: Relayer,
    pub nonce_manager: NonceManager,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    confirmations: u64,
    safe_proxy_manager: Arc<SafeProxyManager>,
    gas_bump_config: GasBumpBlockConfig,
    max_gas_price_multiplier: u64,
}

impl TransactionsQueue {
    pub fn new(
        setup: TransactionsQueueSetup,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
        blob_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    ) -> Self {
        info!(
            "Creating new TransactionsQueue for relayer: {} (name: {}) on chain: {}",
            setup.relayer.id, setup.relayer.name, setup.relayer.chain_id
        );
        let confirmations = setup.evm_provider.confirmations;
        Self {
            pending_transactions: Mutex::new(setup.pending_transactions),
            inmempool_transactions: Mutex::new(setup.inmempool_transactions),
            mined_transactions: Mutex::new(setup.mined_transactions),
            evm_provider: setup.evm_provider,
            relayer: setup.relayer,
            nonce_manager: setup.nonce_manager,
            gas_oracle_cache,
            blob_oracle_cache,
            confirmations,
            safe_proxy_manager: setup.safe_proxy_manager,
            gas_bump_config: setup.gas_bump_config,
            max_gas_price_multiplier: setup.max_gas_price_multiplier,
        }
    }

    fn blocks_to_wait_before_bump(&self, speed: &TransactionSpeed) -> u64 {
        self.gas_bump_config.blocks_to_wait_before_bump(speed)
    }

    pub fn should_bump_gas(&self, ms_between_times: u64, speed: &TransactionSpeed) -> bool {
        let time_threshold_met = ms_between_times
            > (self.evm_provider.blocks_every * self.blocks_to_wait_before_bump(speed));

        if !time_threshold_met {
            return false;
        }

        info!(
            "Gas bump time threshold met for relayer: {} - elapsed: {}ms, threshold: {}ms, speed: {:?}",
            self.relayer.name,
            ms_between_times,
            self.evm_provider.blocks_every * self.blocks_to_wait_before_bump(speed),
            speed
        );

        true
    }

    /// Checks if a transaction has reached the maximum gas price cap and shouldn't be bumped further
    pub async fn is_at_max_gas_price_cap(&self, sent_gas: &GasPriceResult) -> bool {
        // Get SUPER speed gas price for cap calculation
        let super_gas_price = {
            let gas_oracle = self.gas_oracle_cache.lock().await;
            gas_oracle
                .get_gas_price_for_speed(&self.relayer.chain_id, &TransactionSpeed::SUPER)
                .await
        };

        if let Some(super_price) = super_gas_price {
            let max_allowed_max_fee =
                super_price.max_fee.into_u128() * (self.max_gas_price_multiplier as u128);
            let max_allowed_priority_fee =
                super_price.max_priority_fee.into_u128() * (self.max_gas_price_multiplier as u128);

            let at_max_fee_cap =
                max_allowed_max_fee > 0 && sent_gas.max_fee.into_u128() >= max_allowed_max_fee;
            let at_max_priority_fee_cap = max_allowed_priority_fee > 0
                && sent_gas.max_priority_fee.into_u128() >= max_allowed_priority_fee;

            if at_max_fee_cap || at_max_priority_fee_cap {
                info!(
                    "Transaction at maximum gas price cap for relayer: {} - max_fee: {} (cap: {}), max_priority_fee: {} (cap: {})",
                    self.relayer.name,
                    sent_gas.max_fee.into_u128(),
                    max_allowed_max_fee,
                    sent_gas.max_priority_fee.into_u128(),
                    max_allowed_priority_fee
                );
                return true;
            }
        }

        false
    }

    /// Checks if a transaction has reached the maximum blob gas price cap and shouldn't be bumped further
    pub async fn is_at_max_blob_gas_price_cap(&self, sent_blob_gas: &BlobGasPriceResult) -> bool {
        let super_blob_gas_price = {
            let blob_gas_oracle = self.blob_oracle_cache.lock().await;
            blob_gas_oracle
                .get_blob_gas_price_for_speed(&self.relayer.chain_id, &TransactionSpeed::SUPER)
                .await
        };

        if let Some(super_price) = super_blob_gas_price {
            let max_allowed_blob_gas_price =
                super_price.blob_gas_price * (self.max_gas_price_multiplier as u128);

            if max_allowed_blob_gas_price > 0
                && sent_blob_gas.blob_gas_price >= max_allowed_blob_gas_price
            {
                info!(
                    "Transaction at maximum blob gas price cap for relayer: {} - blob_gas_price: {} (cap: {})",
                    self.relayer.name,
                    sent_blob_gas.blob_gas_price,
                    max_allowed_blob_gas_price
                );
                return true;
            }
        }

        false
    }

    pub async fn add_pending_transaction(&mut self, transaction: Transaction) {
        info!(
            "Adding pending transaction {} to queue for relayer: {}",
            transaction.id, self.relayer.name
        );
        let mut transactions = self.pending_transactions.lock().await;
        transactions.push_back(transaction);
        info!(
            "Pending transactions count for relayer {}: {}",
            self.relayer.name,
            transactions.len()
        );
    }

    pub async fn get_next_pending_transaction(&self) -> Option<Transaction> {
        let transactions = self.pending_transactions.lock().await;

        transactions.front().cloned()
    }

    pub async fn get_pending_transaction_count(&self) -> usize {
        let transactions = self.pending_transactions.lock().await;
        let count = transactions.len();
        info!("Current pending transaction count for relayer {}: {}", self.relayer.name, count);
        count
    }

    /// Snapshot of every transaction currently occupying a nonce for this relayer:
    /// pending transactions (nonce reserved, not yet broadcast) in queue order,
    /// followed by inmempool transactions (broadcast, awaiting receipt) including any
    /// same-nonce cancel/replace competitors.
    pub async fn get_inflight_transactions(&self) -> Vec<Transaction> {
        let pending = self.pending_transactions.lock().await;
        let inmempool = self.inmempool_transactions.lock().await;

        snapshot_inflight_transactions(&pending, &inmempool)
    }

    /// Swaps a PENDING transaction's payload for the replacement request IN THE QUEUE
    /// ITSELF and returns the updated transaction for the caller to persist. Returns
    /// `None` when the transaction is no longer pending (it broadcast or was removed
    /// between lookup and edit).
    pub async fn replace_pending_transaction_payload(
        &mut self,
        transaction_id: &TransactionId,
        replace_with: &RelayTransactionRequest,
        new_expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Option<Transaction> {
        let mut pending = self.pending_transactions.lock().await;
        let replaced = replace_pending_transaction_payload_in_queue(
            &mut pending,
            transaction_id,
            replace_with,
            new_expires_at,
        );

        if replaced.is_some() {
            info!(
                "Replaced pending transaction {} payload in place for relayer: {}",
                transaction_id, self.relayer.name
            );
        }

        replaced
    }

    /// Locates the in-flight transaction currently holding a nonce, classifying where
    /// it sits so callers know whether a same-nonce replacement can act on it.
    pub async fn find_inflight_transaction_by_nonce(
        &self,
        nonce: &TransactionNonce,
    ) -> InflightNonceHolder {
        let pending = self.pending_transactions.lock().await;
        let inmempool = self.inmempool_transactions.lock().await;

        find_inflight_transaction_by_nonce(&pending, &inmempool, nonce)
    }

    pub async fn get_editable_transaction_by_id(
        &self,
        id: &TransactionId,
    ) -> Option<EditableTransaction> {
        info!("Looking for editable transaction {} for relayer: {}", id, self.relayer.name);
        let transactions = self.pending_transactions.lock().await;

        let pending = transactions.iter().find(|t| t.id == *id);

        match pending {
            Some(transaction) => {
                info!(
                    "Found transaction {} in pending queue for relayer: {}",
                    id, self.relayer.name
                );
                Some(EditableTransaction::to_pending(transaction.clone()))
            }
            None => {
                let transactions = self.inmempool_transactions.lock().await;
                let result = transactions
                    .iter()
                    .find(|t| t.original.id == *id)
                    .map(|comp_tx| EditableTransaction::to_inmempool(comp_tx.original.clone()));

                if result.is_some() {
                    info!(
                        "Found transaction {} in inmempool queue for relayer: {}",
                        id, self.relayer.name
                    );
                } else {
                    info!(
                        "Transaction {} not found in any queue for relayer: {}",
                        id, self.relayer.name
                    );
                }
                result
            }
        }
    }

    pub async fn move_pending_to_inmempool(
        &mut self,
        transaction: &Transaction,
        transaction_sent: &TransactionSentWithRelayer,
    ) -> Result<(), MovePendingTransactionToInmempoolError> {
        info!(
            "Moving transaction {} from pending to inmempool for relayer: {} with hash: {}",
            transaction_sent.id, self.relayer.name, transaction_sent.hash
        );

        let mut transactions = self.pending_transactions.lock().await;
        let item = transactions.front().cloned();

        if let Some(queued_transaction) = item {
            if queued_transaction.id == transaction_sent.id && transaction.id == transaction_sent.id
            {
                let mut inmempool_transactions = self.inmempool_transactions.lock().await;
                let updated_transaction = Transaction {
                    known_transaction_hash: Some(transaction_sent.hash),
                    status: TransactionStatus::INMEMPOOL,
                    sent_with_max_fee_per_gas: Some(transaction_sent.sent_with_gas.max_fee),
                    sent_with_max_priority_fee_per_gas: Some(
                        transaction_sent.sent_with_gas.max_priority_fee,
                    ),
                    sent_with_gas: Some(transaction_sent.sent_with_gas.clone()),
                    sent_with_blob_gas: transaction_sent.sent_with_blob_gas.clone(),
                    sent_at: Some(Utc::now()),
                    ..transaction.clone()
                };
                inmempool_transactions.push_back(CompetitiveTransaction::new(updated_transaction));

                transactions.pop_front();
                info!("Successfully moved transaction {} to inmempool for relayer: {}. Pending: {}, Inmempool: {}",
                    transaction_sent.id, self.relayer.name, transactions.len(), inmempool_transactions.len());
                Ok(())
            } else {
                info!("Transaction ID mismatch when moving to inmempool for relayer: {}. Expected: {}, Found: {}",
                    self.relayer.name, transaction_sent.id, queued_transaction.id);
                Err(MovePendingTransactionToInmempoolError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    self.relayer.address,
                    transaction_sent.clone(),
                    queued_transaction.clone(),
                ))
            }
        } else {
            info!("No pending transaction found to move to inmempool for relayer: {} (transaction: {})",
                self.relayer.name, transaction_sent.id);
            Err(MovePendingTransactionToInmempoolError::TransactionNotFound(
                self.relayer.id,
                self.relayer.address,
                transaction_sent.clone(),
            ))
        }
    }

    pub async fn remove_pending_transaction_by_id(
        &mut self,
        transaction_id: &TransactionId,
    ) -> bool {
        let mut transactions = self.pending_transactions.lock().await;
        if let Some(pos) = transactions.iter().position(|tx| tx.id == *transaction_id) {
            transactions.remove(pos);
            info!(
                "Removed pending transaction {} from relayer {}: {} remaining",
                transaction_id,
                self.relayer.name,
                transactions.len()
            );
            true
        } else {
            false
        }
    }

    pub async fn update_pending_transaction(&mut self, updated_transaction: Transaction) -> bool {
        let mut transactions = self.pending_transactions.lock().await;
        if let Some(transaction) =
            transactions.iter_mut().find(|tx| tx.id == updated_transaction.id)
        {
            *transaction = updated_transaction;
            info!(
                "Updated pending transaction {} for relayer {}",
                transaction.id, self.relayer.name
            );
            true
        } else {
            false
        }
    }

    pub async fn add_competitor_to_inmempool_transaction(
        &mut self,
        original_transaction_id: &TransactionId,
        competitor_transaction: Transaction,
        competition_type: CompetitionType,
    ) -> Result<(), TransactionQueueSendTransactionError> {
        let mut transactions = self.inmempool_transactions.lock().await;

        if let Some(comp_tx) = transactions.front_mut() {
            if comp_tx.original.id == *original_transaction_id {
                comp_tx.add_competitor(competitor_transaction, competition_type);
                info!(
                    "Added competitor to inmempool transaction {} for relayer {}",
                    original_transaction_id, self.relayer.name
                );
                return Ok(());
            }
        }

        Err(TransactionQueueSendTransactionError::NoTransactionInQueue)
    }

    pub async fn get_next_inmempool_transaction(&self) -> Option<Transaction> {
        let transactions = self.inmempool_transactions.lock().await;

        transactions.front().map(|comp_tx| comp_tx.get_active_transaction().clone())
    }

    pub async fn get_inmempool_transaction_count(&self) -> usize {
        let transactions = self.inmempool_transactions.lock().await;
        let count = transactions.len();
        info!("Current inmempool transaction count for relayer {}: {}", self.relayer.name, count);
        count
    }

    pub async fn update_inmempool_transaction_gas(
        &mut self,
        transaction_sent: &TransactionSentWithRelayer,
    ) {
        let mut transactions = self.inmempool_transactions.lock().await;
        if let Some(comp_tx) = transactions.front_mut() {
            if comp_tx.original.id == transaction_sent.id {
                info!(
                    "Updating inmempool transaction {} with new gas values for relayer: {}",
                    transaction_sent.id, self.relayer.name
                );
                comp_tx.original.known_transaction_hash = Some(transaction_sent.hash);
                comp_tx.original.sent_with_max_fee_per_gas =
                    Some(transaction_sent.sent_with_gas.max_fee);
                comp_tx.original.sent_with_max_priority_fee_per_gas =
                    Some(transaction_sent.sent_with_gas.max_priority_fee);
                comp_tx.original.sent_with_gas = Some(transaction_sent.sent_with_gas.clone());
                comp_tx.original.sent_with_blob_gas = transaction_sent.sent_with_blob_gas.clone();
                comp_tx.original.sent_at = Some(Utc::now());
            } else if let Some((ref mut competitor, _)) = comp_tx.competitive {
                if competitor.id == transaction_sent.id {
                    info!(
                        "Updating competitive transaction {} with new gas values for relayer: {}",
                        transaction_sent.id, self.relayer.name
                    );
                    competitor.known_transaction_hash = Some(transaction_sent.hash);
                    competitor.sent_with_max_fee_per_gas =
                        Some(transaction_sent.sent_with_gas.max_fee);
                    competitor.sent_with_max_priority_fee_per_gas =
                        Some(transaction_sent.sent_with_gas.max_priority_fee);
                    competitor.sent_with_gas = Some(transaction_sent.sent_with_gas.clone());
                    competitor.sent_with_blob_gas = transaction_sent.sent_with_blob_gas.clone();
                    competitor.sent_at = Some(Utc::now());
                }
            }
        }
    }

    pub async fn update_inmempool_transaction_noop(
        &mut self,
        transaction_id: &TransactionId,
        transaction_sent: &TransactionSentWithRelayer,
    ) {
        let mut transactions = self.inmempool_transactions.lock().await;
        if let Some(comp_tx) = transactions.front_mut() {
            if let Some(transaction) = comp_tx.get_transaction_by_id_mut(transaction_id) {
                info!(
                    "Updating inmempool transaction {} with no-op details for relayer: {}",
                    transaction_id, self.relayer.name
                );
                transaction.to = self.relayer.address;
                transaction.value = TransactionValue::zero();
                transaction.data = TransactionData::empty();
                transaction.blobs = None;
                transaction.gas_limit = Some(GasLimit::new(21_000));
                transaction.is_noop = true;
                transaction.speed = TransactionSpeed::FAST;
                transaction.sent_with_blob_gas = None;
                transaction.gas_price_ceiling = None;
                transaction.known_transaction_hash = Some(transaction_sent.hash);
                transaction.sent_at = Some(Utc::now());
            }
        }
    }

    pub async fn update_inmempool_transaction_replaced(
        &mut self,
        transaction_id: &TransactionId,
        transaction_sent_with_relayer: &TransactionSentWithRelayer,
        replacement_transaction: &Transaction,
    ) {
        let mut transactions = self.inmempool_transactions.lock().await;
        if let Some(comp_tx) = transactions.front_mut() {
            if let Some(transaction) = comp_tx.get_transaction_by_id_mut(transaction_id) {
                info!(
                    "Replacing inmempool transaction {} for relayer: {}",
                    transaction_id, self.relayer.name
                );
                transaction.external_id = replacement_transaction.external_id.clone();
                transaction.to = replacement_transaction.to;
                transaction.from = replacement_transaction.from;
                transaction.value = replacement_transaction.value;
                transaction.data = replacement_transaction.data.clone();
                transaction.nonce = replacement_transaction.nonce;
                transaction.speed = replacement_transaction.speed.clone();
                transaction.gas_limit = replacement_transaction.gas_limit;
                transaction.status = replacement_transaction.status;
                transaction.blobs = replacement_transaction.blobs.clone();
                transaction.known_transaction_hash = Some(transaction_sent_with_relayer.hash);
                transaction.queued_at = replacement_transaction.queued_at;
                transaction.expires_at = replacement_transaction.expires_at;
                transaction.sent_at = replacement_transaction.sent_at;
                transaction.sent_with_gas =
                    Some(transaction_sent_with_relayer.sent_with_gas.clone());
                transaction.sent_with_blob_gas =
                    transaction_sent_with_relayer.sent_with_blob_gas.clone();
                transaction.speed = replacement_transaction.speed.clone();
                transaction.sent_with_max_fee_per_gas =
                    replacement_transaction.sent_with_max_fee_per_gas;
                transaction.sent_with_max_priority_fee_per_gas =
                    replacement_transaction.sent_with_max_priority_fee_per_gas;
                transaction.is_noop = replacement_transaction.is_noop;
                transaction.external_id = replacement_transaction.external_id.clone();
                transaction.gas_price_ceiling = replacement_transaction.gas_price_ceiling;
                transaction.gas_price_ceiling_hit = replacement_transaction.gas_price_ceiling_hit;
            }
        }
    }

    pub async fn move_inmempool_to_mining(
        &mut self,
        id: &TransactionId,
        receipt: &AnyTransactionReceipt,
    ) -> Result<CompetitionResolutionResult, MoveInmempoolTransactionToMinedError> {
        info!(
            "Moving transaction {} from inmempool to mined for relayer: {} with receipt status: {}",
            id,
            self.relayer.name,
            receipt.status()
        );

        let mut transactions = self.inmempool_transactions.lock().await;
        let item = transactions.front().cloned();

        if let Some(comp_tx) = item {
            if comp_tx.get_transaction_by_id(id).is_some() {
                let receipt_transaction_status: TransactionStatus;

                if receipt.status() {
                    receipt_transaction_status = TransactionStatus::MINED;
                    info!(
                        "Transaction {} successfully mined for relayer: {}",
                        id, self.relayer.name
                    );
                } else {
                    receipt_transaction_status = TransactionStatus::FAILED;
                    info!("Transaction {} failed on-chain for relayer: {}", id, self.relayer.name);
                }

                let (winner_transaction, loser_transaction, loser_status) = if comp_tx.original.id
                    == *id
                {
                    let loser_status = if let Some((_, comp_type)) = &comp_tx.competitive {
                        match comp_type {
                            CompetitionType::Cancel => TransactionStatus::DROPPED,
                            CompetitionType::Replace => TransactionStatus::DROPPED,
                        }
                    } else {
                        TransactionStatus::DROPPED
                    };

                    // is_noop is derived as to == from when rehydrating from the database,
                    // so also require the noop shape (zero value, empty data) to avoid
                    // misclassifying a genuine user self-send as expired after a restart
                    let noop_payload_shape = comp_tx.original.is_noop
                        && comp_tx.original.value.is_zero()
                        && comp_tx.original.data == TransactionData::empty();
                    let winner_status = if receipt.status()
                        && noop_payload_shape
                        && comp_tx.original.failed_reason.is_some()
                    {
                        // The payload was permanently rejected at send time and replaced
                        // with this no-op purely to consume the reserved nonce - surface
                        // the transaction to the caller as FAILED, not MINED. This holds
                        // even when a cancel/replace competitor lost the race to it.
                        TransactionStatus::FAILED
                    } else if receipt.status()
                        && noop_payload_shape
                        && comp_tx.competitive.is_none()
                        && Self::has_expired(&comp_tx.original)
                    {
                        TransactionStatus::EXPIRED
                    } else {
                        receipt_transaction_status
                    };

                    let winner = Transaction {
                        status: winner_status,
                        mined_at: Some(Utc::now()),
                        cancelled_by_transaction_id: None,
                        ..comp_tx.original
                    };

                    (winner, comp_tx.competitive.map(|(tx, _)| tx), loser_status)
                } else if let Some((competitor, comp_type)) = comp_tx.competitive {
                    let (loser_status, loser_transaction) = match comp_type {
                        CompetitionType::Cancel => {
                            // When cancel wins, original transaction becomes a cancelled no-op
                            let cancelled_original = Transaction {
                                status: TransactionStatus::CANCELLED,
                                is_noop: true,
                                to: self.relay_address(),
                                value: TransactionValue::zero(),
                                data: TransactionData::empty(),
                                cancelled_by_transaction_id: Some(competitor.id),
                                ..comp_tx.original
                            };
                            (TransactionStatus::CANCELLED, cancelled_original)
                        }
                        CompetitionType::Replace => {
                            let replaced_original = Transaction {
                                status: TransactionStatus::REPLACED,
                                ..comp_tx.original
                            };
                            (TransactionStatus::REPLACED, replaced_original)
                        }
                    };

                    let winner = Transaction {
                        status: receipt_transaction_status,
                        mined_at: Some(Utc::now()),
                        ..competitor
                    };

                    (winner, Some(loser_transaction), loser_status)
                } else {
                    return Err(MoveInmempoolTransactionToMinedError::TransactionIdDoesNotMatch(
                        self.relayer.id,
                        self.relayer.address,
                        *id,
                        comp_tx.original,
                    ));
                };

                let mined_count = if winner_transaction.status == TransactionStatus::MINED {
                    let mut mining_transactions = self.mined_transactions.lock().await;
                    mining_transactions.insert(winner_transaction.id, winner_transaction.clone());
                    mining_transactions.len()
                } else {
                    self.mined_transactions.lock().await.len()
                };

                // Log competition resolution but don't put loser transactions in mined queue
                // since they weren't actually mined - they were cancelled/dropped
                let loser_for_result = loser_transaction.clone();
                if let Some(loser) = loser_transaction {
                    info!(
                        "Competition resolved for relayer {} - Winner: {} ({}), Loser: {} ({})",
                        self.relayer.name,
                        winner_transaction.id,
                        winner_transaction.status,
                        loser.id,
                        loser_status
                    );
                } else {
                    info!(
                        "No competition - transaction {} mined normally for relayer: {}",
                        winner_transaction.id, self.relayer.name
                    );
                }

                transactions.pop_front();
                info!("Successfully moved transaction {} to mined status for relayer: {}. Inmempool: {}, Mined: {}",
                    id, self.relayer.name, transactions.len(), mined_count);

                Ok(CompetitionResolutionResult {
                    winner_status: winner_transaction.status,
                    winner: winner_transaction,
                    loser: loser_for_result,
                })
            } else {
                info!(
                    "Transaction ID {} not found in competitive transaction for relayer: {}",
                    id, self.relayer.name
                );
                Err(MoveInmempoolTransactionToMinedError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    self.relayer.address,
                    *id,
                    comp_tx.original,
                ))
            }
        } else {
            info!(
                "No inmempool transaction found to move to mined for relayer: {} (transaction: {})",
                self.relayer.name, id
            );
            Err(MoveInmempoolTransactionToMinedError::TransactionNotFound(
                self.relayer.id,
                self.relayer.address,
                *id,
            ))
        }
    }

    pub async fn get_next_mined_transaction(&self) -> Option<Transaction> {
        let transactions = self.mined_transactions.lock().await;

        if let Some((_, value)) = transactions.iter().next() {
            return Some(value.clone());
        }

        None
    }

    pub async fn is_transaction_mined(&self, id: &TransactionId) -> bool {
        let transactions = self.mined_transactions.lock().await;
        transactions.contains_key(id)
    }

    pub async fn move_mining_to_confirmed(&mut self, id: &TransactionId) {
        info!(
            "Moving transaction {} from mined to confirmed for relayer: {}",
            id, self.relayer.name
        );
        let mut transactions = self.mined_transactions.lock().await;
        transactions.remove(id);
        info!(
            "Successfully confirmed transaction {} for relayer: {}. Remaining mined: {}",
            id,
            self.relayer.name,
            transactions.len()
        );
    }

    pub fn relay_address(&self) -> EvmAddress {
        self.relayer.address
    }

    pub fn relay_id(&self) -> RelayerId {
        self.relayer.id
    }

    pub fn is_legacy_transactions(&self) -> bool {
        !self.relayer.eip_1559_enabled
    }

    pub fn set_is_legacy_transactions(&mut self, is_legacy_transactions: bool) {
        info!(
            "Setting legacy transactions to {} for relayer: {}",
            is_legacy_transactions, self.relayer.name
        );
        self.relayer.eip_1559_enabled = is_legacy_transactions;
    }

    pub fn is_paused(&self) -> bool {
        self.relayer.paused
    }

    pub fn set_is_paused(&mut self, is_paused: bool) {
        info!("Setting paused to {} for relayer: {}", is_paused, self.relayer.name);
        self.relayer.paused = is_paused;
    }

    pub fn set_name(&mut self, name: &str) {
        info!("Changing relayer name from {} to {}", self.relayer.name, name);
        self.relayer.name = name.to_string();
    }

    pub fn max_gas_price(&self) -> Option<GasPrice> {
        self.relayer.max_gas_price
    }

    pub fn set_max_gas_price(&mut self, max_gas_price: Option<GasPrice>) {
        info!("Setting max gas price to {:?} for relayer: {}", max_gas_price, self.relayer.name);
        self.relayer.max_gas_price = max_gas_price;
    }

    pub fn chain_id(&self) -> ChainId {
        self.relayer.chain_id
    }

    pub fn relayer_name(&self) -> &str {
        &self.relayer.name
    }

    /// Returns whether the wallet manager supports EIP-4844 blob transactions
    pub fn supports_blobs(&self) -> bool {
        self.evm_provider.supports_blobs()
    }

    fn within_gas_price_bounds(&self, gas: &GasPriceResult) -> bool {
        if let Some(max) = &self.max_gas_price() {
            let within_bounds = if self.relayer.eip_1559_enabled {
                max.into_u128() >= gas.max_fee.into_u128()
            } else {
                max.into_u128() >= gas.legacy_gas_price().into_u128()
            };

            if !within_bounds {
                info!(
                    "Gas price exceeds bounds for relayer: {}. Max: {}, Proposed: {}",
                    self.relayer.name,
                    max.into_u128(),
                    if self.relayer.eip_1559_enabled {
                        gas.max_fee.into_u128()
                    } else {
                        gas.legacy_gas_price().into_u128()
                    }
                );
            }

            return within_bounds;
        }

        true
    }

    pub fn blocks_every_ms(&self) -> u64 {
        self.evm_provider.blocks_every
    }

    pub fn in_confirmed_range(&self, elapsed: u64) -> bool {
        let threshold = self.blocks_every_ms() * self.confirmations;
        let in_range = elapsed > threshold;
        if in_range {
            info!(
                "Transaction in confirmed range for relayer: {} - elapsed: {}ms, threshold: {}ms",
                self.relayer.name, elapsed, threshold
            );
        }
        in_range
    }

    pub fn has_expired(transaction: &Transaction) -> bool {
        transaction.expires_at < Utc::now()
    }

    pub fn transaction_to_noop(&self, transaction: &mut Transaction) {
        convert_transaction_to_noop(transaction, self.relay_address());
    }

    pub async fn compute_gas_price_for_transaction(
        &self,
        transaction_speed: &TransactionSpeed,
        sent_last_with: Option<&GasPriceResult>,
    ) -> Result<GasPriceResult, SendTransactionGasPriceError> {
        info!(
            "Computing gas price for transaction with speed {:?} for relayer: {}",
            transaction_speed, self.relayer.name
        );

        let mut gas_price = {
            let gas_oracle = self.gas_oracle_cache.lock().await;
            gas_oracle
                .get_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
                .await
                .ok_or(SendTransactionGasPriceError::GasCalculationError)?
        };

        if let Some(sent_gas) = sent_last_with {
            info!("Adjusting gas price based on previous attempt for relayer: {}. Previous max_fee: {}, max_priority_fee: {}",
                self.relayer.name, sent_gas.max_fee.into_u128(), sent_gas.max_priority_fee.into_u128());

            // If we haven't escalated to SUPER speed yet, try to get the next speed level
            if transaction_speed != &TransactionSpeed::SUPER {
                if let Some(next_speed) = transaction_speed.next_speed() {
                    info!(
                        "Using speed escalation for relayer: {} from {:?} to {:?}",
                        self.relayer.name, transaction_speed, next_speed
                    );
                    // Get gas price for the next speed level
                    if let Some(escalated_gas_price) = {
                        let gas_oracle = self.gas_oracle_cache.lock().await;
                        gas_oracle
                            .get_gas_price_for_speed(&self.relayer.chain_id, &next_speed)
                            .await
                    } {
                        gas_price = escalated_gas_price;
                        info!(
                            "Escalated gas price for relayer: {} - max_fee: {}, max_priority_fee: {}",
                            self.relayer.name,
                            gas_price.max_fee.into_u128(),
                            gas_price.max_priority_fee.into_u128()
                        );
                    }
                }
            } else {
                // Already at SUPER speed, do small percentage bumps
                if gas_price.max_fee <= sent_gas.max_fee {
                    let old_max_fee = gas_price.max_fee;
                    gas_price.max_fee = bump_max_fee_by_at_least_one(sent_gas.max_fee);
                    info!(
                        "Small bump max_fee for relayer: {} from {} to {} (5% minimum 1 wei)",
                        self.relayer.name,
                        old_max_fee.into_u128(),
                        gas_price.max_fee.into_u128()
                    );
                }

                if gas_price.max_priority_fee <= sent_gas.max_priority_fee {
                    let old_priority_fee = gas_price.max_priority_fee;
                    gas_price.max_priority_fee =
                        bump_max_priority_fee_by_at_least_one(sent_gas.max_priority_fee);
                    info!(
                        "Small bump max_priority_fee for relayer: {} from {} to {} (5% minimum 1 wei)",
                        self.relayer.name,
                        old_priority_fee.into_u128(),
                        gas_price.max_priority_fee.into_u128()
                    );
                }
            }

            // Ensure we never send a transaction with lower or equal gas prices than previously sent
            if gas_price.max_fee <= sent_gas.max_fee {
                info!(
                    "Escalated max_fee {} is not higher than previously sent {}, using previous + small bump for relayer: {}",
                    gas_price.max_fee.into_u128(),
                    sent_gas.max_fee.into_u128(),
                    self.relayer.name
                );
                gas_price.max_fee = bump_max_fee_by_at_least_one(sent_gas.max_fee);
            }

            if gas_price.max_priority_fee <= sent_gas.max_priority_fee {
                info!(
                    "Escalated max_priority_fee {} is not higher than previously sent {}, using previous + small bump for relayer: {}",
                    gas_price.max_priority_fee.into_u128(),
                    sent_gas.max_priority_fee.into_u128(),
                    self.relayer.name
                );
                gas_price.max_priority_fee =
                    bump_max_priority_fee_by_at_least_one(sent_gas.max_priority_fee);
            }

            // Get SUPER speed gas price for cap calculation
            let super_gas_price = {
                let gas_oracle = self.gas_oracle_cache.lock().await;
                gas_oracle
                    .get_gas_price_for_speed(&self.relayer.chain_id, &TransactionSpeed::SUPER)
                    .await
            };

            if let Some(super_price) = super_gas_price {
                let max_allowed_max_fee =
                    super_price.max_fee.into_u128() * (self.max_gas_price_multiplier as u128);
                let max_allowed_priority_fee = super_price.max_priority_fee.into_u128()
                    * (self.max_gas_price_multiplier as u128);

                if max_allowed_max_fee > 0 && gas_price.max_fee.into_u128() > max_allowed_max_fee {
                    info!(
                        "Gas price max_fee {} exceeds cap {} ({}x SUPER speed), capping for relayer: {}",
                        gas_price.max_fee.into_u128(),
                        max_allowed_max_fee,
                        self.max_gas_price_multiplier,
                        self.relayer.name
                    );
                    gas_price.max_fee = MaxFee::from(max_allowed_max_fee);
                }

                if max_allowed_priority_fee > 0
                    && gas_price.max_priority_fee.into_u128() > max_allowed_priority_fee
                {
                    info!(
                        "Gas price max_priority_fee {} exceeds cap {} ({}x SUPER speed), capping for relayer: {}",
                        gas_price.max_priority_fee.into_u128(),
                        max_allowed_priority_fee,
                        self.max_gas_price_multiplier,
                        self.relayer.name
                    );
                    gas_price.max_priority_fee = MaxPriorityFee::from(max_allowed_priority_fee);
                }
            }

            if gas_price.max_priority_fee.into_u128() > gas_price.max_fee.into_u128() {
                info!(
                    "Adjusted max_priority_fee {} down to max_fee {} for relayer: {}",
                    gas_price.max_priority_fee.into_u128(),
                    gas_price.max_fee.into_u128(),
                    self.relayer.name
                );
                gas_price.max_priority_fee = MaxPriorityFee::from(gas_price.max_fee.into_u128());
            }
        }

        info!(
            "Final gas price for relayer: {} - max_fee: {}, max_priority_fee: {}",
            self.relayer.name,
            gas_price.max_fee.into_u128(),
            gas_price.max_priority_fee.into_u128()
        );

        Ok(gas_price)
    }

    pub async fn compute_blob_gas_price_for_transaction(
        &self,
        transaction_speed: &TransactionSpeed,
        sent_last_with: &Option<BlobGasPriceResult>,
    ) -> Result<BlobGasPriceResult, SendTransactionGasPriceError> {
        info!(
            "Computing blob gas price for transaction with speed {:?} for relayer: {}",
            transaction_speed, self.relayer.name
        );

        let mut blob_gas_price = {
            let blob_gas_oracle = self.blob_oracle_cache.lock().await;
            blob_gas_oracle
                .get_blob_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
                .await
                .ok_or(SendTransactionGasPriceError::BlobGasCalculationError)?
        };

        if let Some(sent_blob_gas) = sent_last_with {
            info!("Adjusting blob gas price based on previous attempt for relayer: {}. Previous blob_gas_price: {}",
                self.relayer.name, sent_blob_gas.blob_gas_price);

            // If we haven't escalated to SUPER speed yet, try to get the next speed level
            if transaction_speed != &TransactionSpeed::SUPER {
                if let Some(next_speed) = transaction_speed.next_speed() {
                    info!(
                        "Using speed escalation for blob gas relayer: {} from {:?} to {:?}",
                        self.relayer.name, transaction_speed, next_speed
                    );
                    if let Some(escalated_blob_gas_price) = {
                        let blob_gas_oracle = self.blob_oracle_cache.lock().await;
                        blob_gas_oracle
                            .get_blob_gas_price_for_speed(&self.relayer.chain_id, &next_speed)
                            .await
                    } {
                        blob_gas_price = escalated_blob_gas_price;
                        info!(
                            "Escalated blob gas price for relayer: {} - blob_gas_price: {}, total_fee: {}",
                            self.relayer.name,
                            blob_gas_price.blob_gas_price,
                            blob_gas_price.total_fee_for_blob
                        );
                    }
                }
            } else {
                // Already at SUPER speed, do small percentage bumps
                if blob_gas_price.blob_gas_price < sent_blob_gas.blob_gas_price {
                    let old_blob_gas_price = blob_gas_price.blob_gas_price;
                    blob_gas_price.blob_gas_price =
                        bump_u128_by_at_least_one(sent_blob_gas.blob_gas_price);
                    blob_gas_price.total_fee_for_blob =
                        blob_gas_price.blob_gas_price * BLOB_GAS_PER_BLOB;

                    info!(
                        "Small bump blob gas price for relayer: {} from {} to {} (5% minimum 1 wei), total_fee: {}",
                        self.relayer.name,
                        old_blob_gas_price,
                        blob_gas_price.blob_gas_price,
                        blob_gas_price.total_fee_for_blob
                    );
                }
            }

            // Ensure we never send a transaction with lower or equal blob gas price than previously sent
            if blob_gas_price.blob_gas_price <= sent_blob_gas.blob_gas_price {
                info!(
                    "Escalated blob gas price {} is not higher than previously sent {}, using previous + small bump for relayer: {}",
                    blob_gas_price.blob_gas_price,
                    sent_blob_gas.blob_gas_price,
                    self.relayer.name
                );
                blob_gas_price.blob_gas_price =
                    bump_u128_by_at_least_one(sent_blob_gas.blob_gas_price);
                blob_gas_price.total_fee_for_blob =
                    blob_gas_price.blob_gas_price * BLOB_GAS_PER_BLOB;
            }

            // Get SUPER speed blob gas price for cap calculation
            let super_blob_gas_price = {
                let blob_gas_oracle = self.blob_oracle_cache.lock().await;
                blob_gas_oracle
                    .get_blob_gas_price_for_speed(&self.relayer.chain_id, &TransactionSpeed::SUPER)
                    .await
            };

            if let Some(super_price) = super_blob_gas_price {
                let max_allowed_blob_gas_price =
                    super_price.blob_gas_price * (self.max_gas_price_multiplier as u128);

                if max_allowed_blob_gas_price > 0
                    && blob_gas_price.blob_gas_price > max_allowed_blob_gas_price
                {
                    info!(
                        "Blob gas price {} exceeds cap {} ({}x SUPER speed), capping for relayer: {}",
                        blob_gas_price.blob_gas_price,
                        max_allowed_blob_gas_price,
                        self.max_gas_price_multiplier,
                        self.relayer.name
                    );
                    blob_gas_price.blob_gas_price = max_allowed_blob_gas_price;
                    blob_gas_price.total_fee_for_blob =
                        blob_gas_price.blob_gas_price * BLOB_GAS_PER_BLOB;
                }
            }
        }

        info!(
            "Final blob gas price for relayer: {} - blob_gas_price: {}, total_fee: {}",
            self.relayer.name, blob_gas_price.blob_gas_price, blob_gas_price.total_fee_for_blob
        );

        Ok(blob_gas_price)
    }

    pub async fn compute_tx_hash(
        &self,
        transaction: &TypedTransaction,
    ) -> Result<TransactionHash, WalletError> {
        info!("Computing transaction hash for relayer: {}", self.relayer.name);

        let signature = self.evm_provider.sign_transaction(&self.relayer, transaction).await?;

        let tx_hash = Self::signed_transaction_hash(transaction, signature);
        info!("Computed transaction hash {} for relayer: {}", tx_hash, self.relayer.name);
        Ok(tx_hash)
    }

    /// Computes the on-chain hash of an already-signed payload without re-signing.
    fn signed_transaction_hash(
        transaction: &TypedTransaction,
        signature: Signature,
    ) -> TransactionHash {
        let hash = match transaction {
            TypedTransaction::Legacy(tx) => {
                let signed = tx.clone().into_signed(signature);
                *signed.hash()
            }
            TypedTransaction::Eip2930(tx) => {
                let signed = tx.clone().into_signed(signature);
                *signed.hash()
            }
            TypedTransaction::Eip1559(tx) => {
                let signed = tx.clone().into_signed(signature);
                *signed.hash()
            }
            TypedTransaction::Eip4844(tx) => {
                let signed = tx.clone().into_signed(signature);
                *signed.hash()
            }
            TypedTransaction::Eip7702(tx) => {
                let signed = tx.clone().into_signed(signature);
                *signed.hash()
            }
        };

        TransactionHash::from_alloy_hash(&hash)
    }

    pub async fn estimate_gas(
        &self,
        transaction_request: &TypedTransaction,
        is_noop: bool,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        info!(
            "Estimating gas for transaction (noop: {}) for relayer: {}",
            is_noop, self.relayer.name
        );

        let estimated_gas_result = self
            .evm_provider
            .estimate_gas(transaction_request, &self.relayer.address)
            .await
            .map_err(|e| {
                error!("Gas estimation failed for relayer {}: {:?}", self.relayer.name, e);
                e
            })?;

        if !is_noop {
            let block_gas_limit = self.evm_provider.get_block_gas_limit().await?;
            if estimated_gas_result > block_gas_limit {
                return Err(RpcError::Transport(TransportErrorKind::Custom(
                    format!(
                        "Estimated gas {} exceeds latest block gas limit {}",
                        estimated_gas_result.into_inner(),
                        block_gas_limit.into_inner()
                    )
                    .into(),
                )));
            }

            let buffered_gas = estimated_gas_result * 12 / 10;
            let estimated_gas = std::cmp::min(buffered_gas, block_gas_limit);
            if estimated_gas < buffered_gas {
                info!(
                    "Gas estimation for relayer: {} - base: {}, buffered: {}, capped to block gas limit: {}",
                    self.relayer.name,
                    estimated_gas_result.into_inner(),
                    buffered_gas.into_inner(),
                    block_gas_limit.into_inner()
                );
                return Ok(estimated_gas);
            }

            info!(
                "Gas estimation for relayer: {} - base: {}, with 20% buffer: {}",
                self.relayer.name,
                estimated_gas_result.into_inner(),
                estimated_gas.into_inner()
            );
            return Ok(estimated_gas);
        }

        info!(
            "Gas estimation for noop transaction for relayer: {} - {}",
            self.relayer.name,
            estimated_gas_result.into_inner()
        );
        Ok(estimated_gas_result)
    }

    /// Advisory only: the cached block gas limit can be stale (or the RPC briefly
    /// down), and a false positive here would burn a reserved nonce on a transaction
    /// the chain would accept. The node's own send rejection ('exceeds block gas
    /// limit') is the authoritative permanent signal.
    async fn warn_if_gas_limit_over_block_cap(&self, gas_limit: GasLimit) {
        match self.evm_provider.get_block_gas_limit().await {
            Ok(block_gas_limit) => {
                if gas_limit > block_gas_limit {
                    error!(
                        "Transaction gas limit {} exceeds latest block gas limit {} for relayer: {} - the node is expected to reject this send",
                        gas_limit.into_inner(),
                        block_gas_limit.into_inner(),
                        self.relayer.name
                    );
                }
            }
            Err(e) => {
                info!(
                    "Could not fetch block gas limit for advisory check on relayer {}: {}",
                    self.relayer.name, e
                );
            }
        }
    }

    /// True when the node's send error proves the submitted payload was rejected and is
    /// definitively not in the mempool - as opposed to transport failures, where the
    /// broadcast may have been accepted with the response lost.
    fn send_error_rules_out_broadcast(error_msg: &str) -> bool {
        matches!(
            classify_send_error(error_msg),
            SendErrorClass::InsufficientFunds
                | SendErrorClass::PermanentRejection
                | SendErrorClass::NonceConflict
                | SendErrorClass::Underpriced
        ) || error_msg.contains("invalid signature")
    }

    /// Records the hash of a signed payload whose broadcast outcome is unknown, in both
    /// the in-memory pending entry and the database, so a later 'nonce too low' receipt
    /// check can recognise the broadcast as our own instead of reassigning its nonce.
    async fn record_broadcast_attempt_hash(
        &self,
        db: &mut PostgresClient,
        transaction: &mut Transaction,
        attempt_hash: TransactionHash,
    ) {
        if transaction.known_transaction_hash == Some(attempt_hash) {
            return;
        }

        info!(
            "Recording broadcast attempt hash {} for transaction {} on relayer: {} (send outcome unknown)",
            attempt_hash, transaction.id, self.relayer.name
        );

        transaction.known_transaction_hash = Some(attempt_hash);

        {
            let mut transactions = self.pending_transactions.lock().await;
            if let Some(stored) = transactions.iter_mut().find(|tx| tx.id == transaction.id) {
                stored.known_transaction_hash = Some(attempt_hash);
            }
        }

        if let Err(db_error) =
            db.transaction_update_known_hash(&transaction.id, &attempt_hash).await
        {
            // In-memory state is already updated; worst case a crash falls back to the
            // previously recorded candidate hash
            error!(
                "Failed to persist broadcast attempt hash for transaction {}: {}",
                transaction.id, db_error
            );
        }
    }

    /// Persists that a transaction's gas price ceiling bound a bid, in both the
    /// in-memory queues and the database, so the flag is visible to status readers
    /// while the transaction is still in flight and survives a restart.
    async fn record_gas_price_ceiling_hit(
        &self,
        db: &mut PostgresClient,
        transaction: &Transaction,
    ) {
        info!(
            "Recording gas price ceiling hit for transaction {} on relayer: {}",
            transaction.id, self.relayer.name
        );

        {
            let mut pending = self.pending_transactions.lock().await;
            if let Some(stored) = pending.iter_mut().find(|tx| tx.id == transaction.id) {
                stored.gas_price_ceiling_hit = true;
            }
        }

        {
            let mut inmempool = self.inmempool_transactions.lock().await;
            for comp_tx in inmempool.iter_mut() {
                if let Some(stored) = comp_tx.get_transaction_by_id_mut(&transaction.id) {
                    stored.gas_price_ceiling_hit = true;
                }
            }
        }

        if let Err(db_error) = db.transaction_update_gas_price_ceiling_hit(&transaction.id).await {
            // In-memory state is already updated; worst case a crash loses the flag
            // until the ceiling binds a bid again after the restart
            error!(
                "Failed to persist gas price ceiling hit for transaction {}: {}",
                transaction.id, db_error
            );
        }
    }

    pub async fn send_transaction(
        &mut self,
        db: &mut PostgresClient,
        transaction: &mut Transaction,
    ) -> Result<TransactionSentWithRelayer, TransactionQueueSendTransactionError> {
        let was_previously_sent = transaction.sent_with_gas.is_some();

        info!(
            "Preparing to send transaction {} for relayer: {} with speed {:?}",
            transaction.id, self.relayer.name, transaction.speed
        );

        info!("Sending transaction {:?} for relayer: {}", transaction, self.relayer.name);

        let mut gas_price = self
            .compute_gas_price_for_transaction(
                &transaction.speed,
                transaction.sent_with_gas.as_ref(),
            )
            .await?;

        // The per-transaction ceiling is not applied to no-ops - including an expired
        // transaction about to be converted below - because the same-nonce close-out
        // MUST be able to broadcast at market price or the reserved nonce would wedge
        // the relayer.
        if !transaction.is_noop && !Self::has_expired(transaction) {
            let ceiling_already_hit = transaction.gas_price_ceiling_hit;
            match transaction.apply_gas_price_ceiling(&mut gas_price) {
                GasPriceCeilingOutcome::WithinCeiling => {}
                outcome => {
                    if !ceiling_already_hit {
                        self.record_gas_price_ceiling_hit(db, transaction).await;
                    }

                    if outcome == GasPriceCeilingOutcome::BlockedByCeiling {
                        info!(
                            "Transaction {} gas price ceiling reached - keeping the last compliant bid for relayer: {}",
                            transaction.id, self.relayer.name
                        );
                        return Err(TransactionQueueSendTransactionError::GasPriceCeilingReached);
                    }

                    info!(
                        "Transaction {} bid clamped to its gas price ceiling for relayer: {}",
                        transaction.id, self.relayer.name
                    );
                }
            }
        }
        let gas_price = gas_price;

        if !self.within_gas_price_bounds(&gas_price) {
            info!(
                "Transaction {} rejected - gas price too high for relayer: {}",
                transaction.id, self.relayer.name
            );
            return Err(TransactionQueueSendTransactionError::GasPriceTooHigh);
        }

        let safe_proxy_address = if transaction.is_noop {
            None
        } else {
            self.safe_proxy_manager
                .get_safe_proxy_for_relayer(&self.relayer.address, transaction.chain_id)
        };

        let (final_to, final_data) = if let Some(safe_address) = safe_proxy_address {
            info!(
                "Routing transaction {} through safe proxy {} for relayer: {}",
                transaction.id, safe_address, self.relayer.name
            );

            let safe_nonce =
                self.safe_proxy_manager.get_safe_nonce(&self.evm_provider, &safe_address).await?;

            let (safe_addr, safe_tx) = self
                .safe_proxy_manager
                .wrap_transaction_for_safe(
                    &self.relayer.address,
                    transaction.chain_id,
                    transaction.to,
                    transaction.value,
                    transaction.data.clone(),
                    safe_nonce,
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;

            let safe_tx_hash = self
                .safe_proxy_manager
                .get_safe_transaction_hash(&safe_addr, &safe_tx, self.evm_provider.chain_id.u64())
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;

            let hash_hex = format!("0x{}", hex::encode(safe_tx_hash));

            let signature =
                self.evm_provider.sign_text(&self.relayer, &hash_hex).await.map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(format!(
                        "Failed to sign safe transaction hash: {}",
                        e
                    ))
                })?;

            // Encode the signature into bytes according to Safe's requirements
            // Safe signature format: r + s + v where v = recovery_id + 4
            let mut sig_bytes = Vec::with_capacity(65);
            sig_bytes.extend_from_slice(&signature.r().to_be_bytes::<32>());
            sig_bytes.extend_from_slice(&signature.s().to_be_bytes::<32>());
            // Safe requires v = recovery_id + 4 for ECDSA signatures
            let recovery_id = if signature.v() { 1u8 } else { 0u8 };
            sig_bytes.push(recovery_id + 4);
            let signatures = alloy::primitives::Bytes::from(sig_bytes);

            let safe_call_data = self
                .safe_proxy_manager
                .encode_safe_transaction(&safe_tx, signatures)
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;

            (safe_addr, TransactionData::new(safe_call_data))
        } else {
            (transaction.to, transaction.data.clone())
        };

        let mut working_transaction = transaction.clone();
        working_transaction.to = final_to;
        working_transaction.data = final_data;

        // If using safe proxy, the transaction value should be 0 because the ETH transfer
        // amount is encoded in the execTransaction call data, not in the transaction value
        if safe_proxy_address.is_some() {
            working_transaction.value = TransactionValue::zero();
        }

        // Estimate gas limit by creating a temporary transaction with a high gas limit to avoid failing the estimate
        let temp_gas_limit = GasLimit::new(10_000_000);

        let temp_transaction_request = if working_transaction.is_blob_transaction() {
            info!(
                "Creating blob transaction for gas estimation for relayer: {}",
                self.relayer.name
            );
            let blob_gas_price = self
                .compute_blob_gas_price_for_transaction(
                    &working_transaction.speed,
                    &working_transaction.sent_with_blob_gas,
                )
                .await?;
            working_transaction
                .to_blob_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(&blob_gas_price),
                    Some(temp_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        } else if self.is_legacy_transactions() {
            info!(
                "Creating legacy transaction for gas estimation for relayer: {}",
                self.relayer.name
            );
            working_transaction
                .to_legacy_typed_transaction_with_gas_limit(Some(&gas_price), Some(temp_gas_limit))
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        } else {
            info!(
                "Creating EIP-1559 transaction for gas estimation for relayer: {}",
                self.relayer.name
            );
            working_transaction
                .to_eip1559_typed_transaction_with_gas_limit(Some(&gas_price), Some(temp_gas_limit))
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        };

        let mut estimated_gas_limit = if let Some(gas_limit) = transaction.gas_limit {
            gas_limit
        } else {
            self.estimate_gas(&temp_transaction_request, working_transaction.is_noop)
                .await
                .map_err(TransactionQueueSendTransactionError::TransactionEstimateGasError)?
        };

        if safe_proxy_address.is_some() {
            let original_estimate = estimated_gas_limit;

            // Safe proxy gas overhead calculation:
            // Test data shows: Failed at 25k and 37k gas, succeeded at 65k gas
            // Safe execTransaction overhead includes:
            // - Signature verification (~5-15k gas per signature)
            // - Safe contract state checks (~5-10k gas)
            // - Payment/refund logic (~5-10k gas)
            // - Event emission (~5k gas)
            // Total overhead: ~20-40k gas minimum

            // Add 45k gas overhead to base estimate to be safe and cater for the overhead
            let safe_overhead = GasLimit::new(45_000);
            estimated_gas_limit = estimated_gas_limit + safe_overhead;

            info!(
                "Applied Safe proxy gas overhead for relayer: {} - original: {}, overhead: {}, final: {}",
                self.relayer.name,
                original_estimate.into_inner(),
                safe_overhead.into_inner(),
                estimated_gas_limit.into_inner()
            );
        }

        self.warn_if_gas_limit_over_block_cap(estimated_gas_limit).await;

        working_transaction.gas_limit = Some(estimated_gas_limit);
        transaction.gas_limit = Some(estimated_gas_limit);

        let (mut transaction_request, mut sent_with_blob_gas): (
            TypedTransaction,
            Option<BlobGasPriceResult>,
        ) = if working_transaction.is_blob_transaction() {
            info!("Creating final blob transaction for relayer: {}", self.relayer.name);
            let blob_gas_price = self
                .compute_blob_gas_price_for_transaction(
                    &working_transaction.speed,
                    &working_transaction.sent_with_blob_gas,
                )
                .await?;
            let tx_request = working_transaction
                .to_blob_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(&blob_gas_price),
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;
            (tx_request, Some(blob_gas_price))
        } else if self.is_legacy_transactions() {
            info!("Creating final legacy transaction for relayer: {}", self.relayer.name);
            let tx_request = working_transaction
                .to_legacy_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;
            (tx_request, None)
        } else {
            info!("Creating final EIP-1559 transaction for relayer: {}", self.relayer.name);
            let tx_request = working_transaction
                .to_eip1559_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?;
            (tx_request, None)
        };
        info!(
            "Set gas limit {} for transaction {} on relayer: {}",
            estimated_gas_limit.into_inner(),
            transaction.id,
            self.relayer.name
        );

        // A broadcast blob transaction can only be replaced by another blob transaction
        // (geth/reth reject cross-type replacements at the same nonce), so an expired one
        // must keep being bumped as-is instead of being converted to a no-op
        let can_replace_with_noop = !was_previously_sent || !transaction.is_blob_transaction();

        if !transaction.is_noop && can_replace_with_noop && Self::has_expired(transaction) {
            info!(
                "Transaction {} expired before broadcast for relayer: {}, sending no-op replacement",
                transaction.id, self.relayer.name
            );

            self.transaction_to_noop(transaction);
            working_transaction = transaction.clone();
            sent_with_blob_gas = None;

            transaction_request = if self.is_legacy_transactions() {
                working_transaction
                    .to_legacy_typed_transaction_with_gas_limit(
                        Some(&gas_price),
                        Some(GasLimit::new(21_000)),
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?
            } else {
                working_transaction
                    .to_eip1559_typed_transaction_with_gas_limit(
                        Some(&gas_price),
                        Some(GasLimit::new(21_000)),
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?
            };
        }

        info!(
            "Sending transaction {:?} to network for relayer: {}",
            transaction_request, self.relayer.name
        );

        let mut signature =
            self.evm_provider.sign_transaction(&self.relayer, &transaction_request).await.map_err(
                |e| {
                    TransactionQueueSendTransactionError::TransactionSendError(
                        SendTransactionError::InternalError(e.to_string()),
                    )
                },
            )?;

        if !transaction.is_noop && can_replace_with_noop && Self::has_expired(transaction) {
            info!(
                "Transaction {} expired after signing for relayer: {}, signing no-op replacement",
                transaction.id, self.relayer.name
            );

            self.transaction_to_noop(transaction);
            working_transaction = transaction.clone();
            sent_with_blob_gas = None;

            transaction_request = if self.is_legacy_transactions() {
                working_transaction
                    .to_legacy_typed_transaction_with_gas_limit(
                        Some(&gas_price),
                        Some(GasLimit::new(21_000)),
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?
            } else {
                working_transaction
                    .to_eip1559_typed_transaction_with_gas_limit(
                        Some(&gas_price),
                        Some(GasLimit::new(21_000)),
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?
            };

            signature = self
                .evm_provider
                .sign_transaction(&self.relayer, &transaction_request)
                .await
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionSendError(
                        SendTransactionError::InternalError(e.to_string()),
                    )
                })?;
        }

        let attempt_hash = Self::signed_transaction_hash(&transaction_request, signature);

        let transaction_hash =
            match self.evm_provider.send_signed_transaction(transaction_request, signature).await {
                Ok(hash) => hash,
                Err(error) => {
                    // A transport-level failure is ambiguous: the node may have accepted the
                    // broadcast even though the response was lost ('already known' proves it
                    // did). Record the hash of the exact signed payload we attempted so the
                    // 'nonce too low' receipt check can recognise the broadcast as our own if
                    // it mines. A definitive node rejection means this payload is NOT in the
                    // mempool, so the previously recorded candidate must be kept.
                    if !was_previously_sent
                        && !Self::send_error_rules_out_broadcast(&error.to_string().to_lowercase())
                    {
                        self.record_broadcast_attempt_hash(db, transaction, attempt_hash).await;
                    }
                    return Err(TransactionQueueSendTransactionError::TransactionSendError(error));
                }
            };

        let transaction_sent = TransactionSentWithRelayer {
            id: transaction.id,
            hash: transaction_hash,
            sent_with_gas: gas_price,
            sent_with_blob_gas,
        };

        transaction.known_transaction_hash = Some(transaction_sent.hash);
        transaction.sent_with_max_fee_per_gas = Some(transaction_sent.sent_with_gas.max_fee);
        transaction.sent_with_max_priority_fee_per_gas =
            Some(transaction_sent.sent_with_gas.max_priority_fee);
        transaction.sent_with_gas = Some(transaction_sent.sent_with_gas.clone());
        transaction.sent_with_blob_gas = transaction_sent.sent_with_blob_gas.clone();
        transaction.sent_at = Some(Utc::now());
        transaction.status = TransactionStatus::INMEMPOOL;

        info!(
            "Transaction {} sent successfully with hash {} for relayer: {}",
            transaction_sent.id, transaction_sent.hash, self.relayer.name
        );

        if !was_previously_sent || transaction.is_noop {
            info!(
                "Updating database for sent transaction {} on relayer: {}",
                transaction.id, self.relayer.name
            );
            // Persist the no-op fields before marking the transaction as sent so a crash
            // between the two commits leaves a pending no-op row (safe to resend) rather
            // than an inmempool row that still carries the original payload
            if transaction.is_noop {
                db.update_transaction_noop(
                    &transaction.id,
                    &transaction.to,
                    &transaction.speed,
                    &transaction.gas_limit.unwrap_or(GasLimit::new(21_000)),
                )
                .await?;
            }

            if !was_previously_sent {
                db.transaction_sent(
                    &transaction_sent.id,
                    &transaction_sent.hash,
                    &transaction_sent.sent_with_gas,
                    transaction_sent.sent_with_blob_gas.as_ref(),
                    self.is_legacy_transactions(),
                )
                .await?;
            }
        } else {
            info!(
                "Skipping DB update for gas bump transaction {} on relayer: {}",
                transaction.id, self.relayer.name
            );
        }

        info!(
            "Successfully processed transaction {} for relayer: {}",
            transaction.id, self.relayer.name
        );
        Ok(transaction_sent)
    }

    pub async fn get_receipt(
        &mut self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<AnyTransactionReceipt>, RpcError<TransportErrorKind>> {
        info!(
            "Getting receipt for transaction hash {} on relayer: {}",
            transaction_hash, self.relayer.name
        );
        let receipt = self.evm_provider.get_receipt(transaction_hash).await?;

        if receipt.is_some() {
            info!(
                "Receipt found for transaction hash {} on relayer: {}",
                transaction_hash, self.relayer.name
            );
        } else {
            info!(
                "No receipt found for transaction hash {} on relayer: {}",
                transaction_hash, self.relayer.name
            );
        }

        Ok(receipt)
    }

    pub async fn get_nonce(&self) -> Result<TransactionNonce, RpcError<TransportErrorKind>> {
        let nonce = self.evm_provider.get_nonce_from_address(&self.relay_address()).await?;

        Ok(nonce)
    }

    /// The MINED nonce (`latest` tag, never counting mempool contents) - the honest
    /// answer to "was this nonce consumed on chain". [`Self::get_nonce`] uses the
    /// `pending` tag, which counts this relayer's own broadcast-but-unmined
    /// transactions, so it must never gate replacing/cancelling one of them.
    pub async fn get_mined_nonce(&self) -> Result<TransactionNonce, RpcError<TransportErrorKind>> {
        let nonce = self.evm_provider.get_mined_nonce_from_address(&self.relay_address()).await?;

        Ok(nonce)
    }

    pub async fn get_balance(
        &self,
    ) -> Result<alloy::primitives::U256, RpcError<TransportErrorKind>> {
        let address = self.relay_address();
        self.evm_provider.get_balance(&address).await
    }

    pub async fn update_pending_transaction_nonce(
        &self,
        transaction_id: &TransactionId,
        new_nonce: TransactionNonce,
    ) {
        let mut pending = self.pending_transactions.lock().await;
        if let Some(transaction) = pending.iter_mut().find(|tx| tx.id == *transaction_id) {
            transaction.nonce = new_nonce;
        }
    }

    pub async fn update_inmempool_transaction_nonce(
        &self,
        transaction_id: &TransactionId,
        new_nonce: TransactionNonce,
    ) {
        let mut inmempool = self.inmempool_transactions.lock().await;
        if let Some(competitive_tx) =
            inmempool.iter_mut().find(|ctx| ctx.get_transaction_by_id(transaction_id).is_some())
        {
            if let Some(transaction) = competitive_tx.get_transaction_by_id_mut(transaction_id) {
                transaction.nonce = new_nonce;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{
        classify_send_error, find_inflight_transaction_by_nonce,
        replace_pending_transaction_payload_in_queue, snapshot_inflight_transactions,
        CompetitiveTransaction, InflightNonceHolder, RelayTransactionRequest, SendErrorClass,
    };
    use crate::transaction::queue_system::types::CompetitionType;
    use crate::transaction::types::{
        test_fixtures::test_transaction, TransactionNonce, TransactionStatus,
    };

    #[test]
    fn classify_send_error_covers_node_wordings() {
        let cases: Vec<(&str, SendErrorClass)> = vec![
            // Permanent rejections - checked before funds because revert reasons
            // routinely contain the word 'balance'
            (
                "execution reverted: erc20: transfer amount exceeds balance",
                SendErrorClass::PermanentRejection,
            ),
            (
                "execution reverted: ownable: caller is not the owner",
                SendErrorClass::PermanentRejection,
            ),
            ("execution reverted", SendErrorClass::PermanentRejection),
            // Revert reasons quoting nonce/mempool wording must NOT be mistaken
            // for node-level nonce conflicts - resynchronising would re-broadcast
            // a permanently reverting payload forever
            ("execution reverted: invalid nonce", SendErrorClass::PermanentRejection),
            ("execution reverted: fwd: nonce too low", SendErrorClass::PermanentRejection),
            ("invalid opcode: opcode 0xfe not defined", SendErrorClass::PermanentRejection),
            ("intrinsic gas too low", SendErrorClass::PermanentRejection),
            ("exceeds block gas limit", SendErrorClass::PermanentRejection),
            // Size-cap rejections (geth's oversized data / EIP-3860 initcode cap,
            // nethermind's MaxTxSizeExceeded) can never succeed on resend
            ("oversized data", SendErrorClass::PermanentRejection),
            ("max initcode size exceeded", SendErrorClass::PermanentRejection),
            ("maxtxsizeexceeded", SendErrorClass::PermanentRejection),
            // Operator-fixable funding conditions - including geth's balance-capped
            // estimation wording, which must NOT be treated as a payload defect
            ("insufficient funds for gas * price + value", SendErrorClass::InsufficientFunds),
            ("gas required exceeds allowance (21000)", SendErrorClass::InsufficientFunds),
            ("insufficient funds for transfer", SendErrorClass::InsufficientFunds),
            ("insufficientfunds, balance is too low", SendErrorClass::InsufficientFunds),
            ("overshot 5000", SendErrorClass::InsufficientFunds),
            // Mempool-presence signals across client wordings
            ("already known", SendErrorClass::AlreadyKnown),
            ("alreadyknown", SendErrorClass::AlreadyKnown),
            ("known transaction: 0xabc", SendErrorClass::AlreadyKnown),
            ("transaction with the same hash was already imported", SendErrorClass::AlreadyKnown),
            // Nonce conflicts across client wordings
            ("nonce too low", SendErrorClass::NonceConflict),
            ("nonce is too low", SendErrorClass::NonceConflict),
            ("invalid nonce", SendErrorClass::NonceConflict),
            ("nonce has already been used", SendErrorClass::NonceConflict),
            ("oldnonce", SendErrorClass::NonceConflict),
            // Fee-bump rejections keep the existing broadcast alive
            ("replacement transaction underpriced", SendErrorClass::Underpriced),
            ("transaction underpriced", SendErrorClass::Underpriced),
            ("feetoolow", SendErrorClass::Underpriced),
            ("feetoolowtocompete", SendErrorClass::Underpriced),
            // Unknown wording / transport failures must stay retryable - never
            // close out (which burns the nonce) on an unrecognised string
            ("connection refused", SendErrorClass::Transient),
            ("request timed out", SendErrorClass::Transient),
            ("load balancer error 502", SendErrorClass::Transient),
            ("txpool is full", SendErrorClass::Transient),
        ];

        for (message, expected) in cases {
            assert_eq!(
                classify_send_error(message),
                expected,
                "misclassified node error: {message}"
            );
        }
    }

    #[test]
    fn inflight_snapshot_lists_pending_then_inmempool_with_competitors() {
        let pending_first = test_transaction(12, TransactionStatus::PENDING);
        let pending_second = test_transaction(13, TransactionStatus::PENDING);
        let inmempool_original = test_transaction(10, TransactionStatus::INMEMPOOL);
        let mut competitive = CompetitiveTransaction::new(inmempool_original.clone());
        let competitor = test_transaction(10, TransactionStatus::INMEMPOOL);
        competitive.add_competitor(competitor.clone(), CompetitionType::Cancel);
        let inmempool_plain =
            CompetitiveTransaction::new(test_transaction(11, TransactionStatus::INMEMPOOL));

        let pending: VecDeque<_> = vec![pending_first.clone(), pending_second.clone()].into();
        let inmempool: VecDeque<_> = vec![competitive, inmempool_plain.clone()].into();

        let snapshot = snapshot_inflight_transactions(&pending, &inmempool);

        let ids: Vec<_> = snapshot.iter().map(|tx| tx.id).collect();
        assert_eq!(
            ids,
            vec![
                pending_first.id,
                pending_second.id,
                inmempool_original.id,
                competitor.id,
                inmempool_plain.original.id
            ]
        );

        let nonces: Vec<u64> = snapshot.iter().map(|tx| tx.nonce.into_inner()).collect();
        assert_eq!(nonces, vec![12, 13, 10, 10, 11]);
    }

    #[test]
    fn inflight_snapshot_of_empty_queues_is_empty() {
        let snapshot = snapshot_inflight_transactions(&VecDeque::new(), &VecDeque::new());
        assert!(snapshot.is_empty());
    }

    #[test]
    fn find_by_nonce_classifies_pending_head_and_behind_head() {
        let pending_tx = test_transaction(12, TransactionStatus::PENDING);
        let head = CompetitiveTransaction::new(test_transaction(10, TransactionStatus::INMEMPOOL));
        let behind =
            CompetitiveTransaction::new(test_transaction(11, TransactionStatus::INMEMPOOL));

        let pending: VecDeque<_> = vec![pending_tx.clone()].into();
        let inmempool: VecDeque<_> = vec![head.clone(), behind.clone()].into();

        match find_inflight_transaction_by_nonce(&pending, &inmempool, &TransactionNonce::new(12)) {
            InflightNonceHolder::Pending(tx) => assert_eq!(tx.id, pending_tx.id),
            other => panic!("nonce 12 should be pending, got {other:?}"),
        }

        match find_inflight_transaction_by_nonce(&pending, &inmempool, &TransactionNonce::new(10)) {
            InflightNonceHolder::InmempoolHead(tx) => assert_eq!(tx.id, head.original.id),
            other => panic!("nonce 10 should be the inmempool head, got {other:?}"),
        }

        match find_inflight_transaction_by_nonce(&pending, &inmempool, &TransactionNonce::new(11)) {
            InflightNonceHolder::InmempoolBehindHead(tx) => assert_eq!(tx.id, behind.original.id),
            other => panic!("nonce 11 should be behind the head, got {other:?}"),
        }

        assert!(matches!(
            find_inflight_transaction_by_nonce(&pending, &inmempool, &TransactionNonce::new(99)),
            InflightNonceHolder::NotFound
        ));
    }

    // Regression test: replacing a PENDING transaction must swap the payload in the
    // queue entry itself - editing only a looked-up clone left the original payload
    // queued for broadcast while the replace reported success.
    #[test]
    fn pending_replace_swaps_the_payload_in_the_queue_itself() {
        use crate::gas::GasLimit;
        use crate::shared::common_types::EvmAddress;
        use crate::transaction::types::{TransactionData, TransactionHash, TransactionValue};
        use std::str::FromStr;

        let mut original = test_transaction(7, TransactionStatus::PENDING);
        original.gas_limit = Some(GasLimit::new(21_000));
        original.known_transaction_hash = Some(
            TransactionHash::from_str(
                "0x1111111111111111111111111111111111111111111111111111111111111111",
            )
            .expect("valid test hash"),
        );

        let new_to = EvmAddress::from(alloy::primitives::Address::repeat_byte(0x33));
        let replace_with = RelayTransactionRequest {
            to: new_to,
            value: TransactionValue::new(alloy::primitives::U256::from(42u64)),
            data: TransactionData::new(vec![0xde, 0xad].into()),
            speed: None,
            external_id: Some("replaced".to_string()),
            blobs: None,
            gas_price_ceiling: None,
            expires_in_seconds: None,
        };

        let mut pending: VecDeque<_> = vec![original.clone()].into();

        let updated = replace_pending_transaction_payload_in_queue(
            &mut pending,
            &original.id,
            &replace_with,
            None,
        )
        .expect("the pending holder should be replaceable");

        // Only one transaction remains queued at the nonce - nothing extra broadcasts
        assert_eq!(pending.len(), 1);

        // THE QUEUE'S OWN ENTRY carries the replacement payload, not just the returned copy
        let queued = pending.front().expect("queue should still hold the transaction");
        assert_eq!(queued.id, original.id);
        assert_eq!(queued.nonce, original.nonce);
        assert_eq!(queued.to, new_to);
        assert_eq!(queued.value, replace_with.value);
        assert_eq!(queued.data, replace_with.data);
        assert_eq!(queued.external_id, Some("replaced".to_string()));
        // Gas limit re-estimates and the stale precomputed hash is dropped
        assert_eq!(queued.gas_limit, None);
        assert_eq!(queued.known_transaction_hash, None);

        // The returned transaction is the persisted shape - identical to the queue entry
        assert_eq!(updated.to, queued.to);
        assert_eq!(updated.known_transaction_hash, None);

        // Unknown ids leave the queue untouched
        let missing = replace_pending_transaction_payload_in_queue(
            &mut pending,
            &test_transaction(8, TransactionStatus::PENDING).id,
            &replace_with,
            None,
        );
        assert!(missing.is_none());
    }

    // A transaction expired by its per-transaction expiry goes through the normal
    // same-nonce no-op close-out, and the close-out stays exempt from the gas price
    // ceiling (cleared) while the hit flag survives for status readers.
    #[test]
    fn per_transaction_expiry_closes_out_through_the_noop_machinery() {
        use super::convert_transaction_to_noop;
        use crate::gas::GasPrice;
        use crate::shared::common_types::EvmAddress;
        use crate::transaction::types::{
            GasPriceCeiling, GasPriceCeilingBehavior, TransactionData,
        };
        use chrono::Utc;

        let relayer_address = EvmAddress::from(alloy::primitives::Address::repeat_byte(0x11));

        let mut transaction = test_transaction(5, TransactionStatus::INMEMPOOL);
        // Queued 10 minutes ago with a 300 second per-transaction expiry - the
        // deadline passed 5 minutes ago even though the 12h global window has not
        transaction.queued_at = Utc::now() - chrono::Duration::minutes(10);
        transaction.expires_at = transaction.queued_at + chrono::Duration::seconds(300);
        transaction.gas_price_ceiling = Some(GasPriceCeiling {
            max_price: GasPrice::new(100),
            behavior: GasPriceCeilingBehavior::Freeze,
        });
        transaction.gas_price_ceiling_hit = true;

        // The expiry sweep sees it as stale...
        assert!(super::TransactionsQueue::has_expired(&transaction));

        // ...and the close-out converts it to the same-nonce no-op
        convert_transaction_to_noop(&mut transaction, relayer_address);

        assert!(transaction.is_noop);
        assert_eq!(transaction.to, relayer_address);
        assert!(transaction.value.is_zero());
        assert_eq!(transaction.data, TransactionData::empty());
        assert_eq!(transaction.blobs, None);
        // The ceiling is exempt for the close-out so the nonce always clears at
        // market price, while the hit flag survives to mark the ceiling-bound expiry
        assert_eq!(transaction.gas_price_ceiling, None);
        assert!(transaction.gas_price_ceiling_hit);
    }
}
