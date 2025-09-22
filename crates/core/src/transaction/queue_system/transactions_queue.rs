use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use super::types::{
    EditableTransaction, MoveInmempoolTransactionToMinedError,
    MovePendingTransactionToInmempoolError, SendTransactionGasPriceError,
    TransactionQueueSendTransactionError, TransactionSentWithRelayer, TransactionsQueueSetup,
};
use crate::transaction::types::{TransactionNonce, TransactionValue};
use crate::{
    gas::{
        BlobGasOracleCache, BlobGasPriceResult, GasLimit, GasOracleCache, GasPrice, GasPriceResult,
        BLOB_GAS_PER_BLOB,
    },
    network::ChainId,
    postgres::PostgresClient,
    provider::EvmProvider,
    relayer::{Relayer, RelayerId},
    safe_proxy::SafeProxyManager,
    shared::common_types::EvmAddress,
    transaction::types::TransactionData,
    transaction::{
        nonce_manager::NonceManager,
        types::{Transaction, TransactionHash, TransactionId, TransactionSpeed, TransactionStatus},
    },
    WalletError,
};
use alloy::network::{AnyTransactionReceipt, ReceiptResponse};
use alloy::{
    consensus::{SignableTransaction, TypedTransaction},
    hex,
    signers::local::LocalSignerError,
    transports::{RpcError, TransportErrorKind},
};
use alloy_eips::{BlockId, BlockNumberOrTag};
use chrono::Utc;
use tokio::sync::Mutex;
use tracing::error;
use tracing::log::info;

pub struct TransactionsQueue {
    pending_transactions: Mutex<VecDeque<Transaction>>,
    inmempool_transactions: Mutex<VecDeque<Transaction>>,
    mined_transactions: Mutex<HashMap<TransactionId, Transaction>>,
    evm_provider: EvmProvider,
    relayer: Relayer,
    pub nonce_manager: NonceManager,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    confirmations: u64,
    safe_proxy_manager: Option<SafeProxyManager>,
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
        }
    }

    fn blocks_to_wait_before_bump(&self, speed: &TransactionSpeed) -> u64 {
        match speed {
            TransactionSpeed::Slow => 10,
            TransactionSpeed::Medium => 5,
            TransactionSpeed::Fast => 4,
            TransactionSpeed::Super => 2,
        }
    }

    pub fn should_bump_gas(&self, ms_between_times: u64, speed: &TransactionSpeed) -> bool {
        let should_bump = ms_between_times
            > (self.evm_provider.blocks_every * self.blocks_to_wait_before_bump(speed));
        if should_bump {
            info!(
                "Gas bump required for relayer: {} - elapsed: {}ms, threshold: {}ms, speed: {:?}",
                self.relayer.name,
                ms_between_times,
                self.evm_provider.blocks_every * self.blocks_to_wait_before_bump(speed),
                speed
            );
        }
        should_bump
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
                    .find(|t| t.id == *id)
                    .map(|transaction| EditableTransaction::to_inmempool(transaction.clone()));

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
        transaction_sent: &TransactionSentWithRelayer,
    ) -> Result<(), MovePendingTransactionToInmempoolError> {
        info!(
            "Moving transaction {} from pending to inmempool for relayer: {} with hash: {}",
            transaction_sent.id, self.relayer.name, transaction_sent.hash
        );

        let mut transactions = self.pending_transactions.lock().await;
        let item = transactions.front().cloned();

        if let Some(transaction) = item {
            if transaction.id == transaction_sent.id {
                let mut inmempool_transactions = self.inmempool_transactions.lock().await;
                inmempool_transactions.push_back(Transaction {
                    known_transaction_hash: Some(transaction_sent.hash),
                    status: TransactionStatus::Inmempool,
                    sent_with_max_fee_per_gas: Some(transaction_sent.sent_with_gas.max_fee),
                    sent_with_max_priority_fee_per_gas: Some(
                        transaction_sent.sent_with_gas.max_priority_fee,
                    ),
                    sent_with_gas: Some(transaction_sent.sent_with_gas.clone()),
                    sent_at: Some(Utc::now()),
                    ..transaction
                });

                transactions.pop_front();
                info!("Successfully moved transaction {} to inmempool for relayer: {}. Pending: {}, Inmempool: {}",
                    transaction_sent.id, self.relayer.name, transactions.len(), inmempool_transactions.len());
                Ok(())
            } else {
                info!("Transaction ID mismatch when moving to inmempool for relayer: {}. Expected: {}, Found: {}",
                    self.relayer.name, transaction_sent.id, transaction.id);
                Err(MovePendingTransactionToInmempoolError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    transaction_sent.clone(),
                    transaction.clone(),
                ))
            }
        } else {
            info!("No pending transaction found to move to inmempool for relayer: {} (transaction: {})",
                self.relayer.name, transaction_sent.id);
            Err(MovePendingTransactionToInmempoolError::TransactionNotFound(
                self.relayer.id,
                transaction_sent.clone(),
            ))
        }
    }

    pub async fn move_next_pending_to_failed(&mut self) {
        let mut transactions = self.pending_transactions.lock().await;
        if let Some(tx) = transactions.front() {
            info!(
                "Moving pending transaction {} to failed for relayer: {}",
                tx.id, self.relayer.name
            );
        }
        transactions.pop_front();
        info!(
            "Remaining pending transactions for relayer {}: {}",
            self.relayer.name,
            transactions.len()
        );
    }

    pub async fn get_next_inmempool_transaction(&self) -> Option<Transaction> {
        let transactions = self.inmempool_transactions.lock().await;

        transactions.front().cloned()
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
        if let Some(transaction) = transactions.front_mut() {
            if transaction.id == transaction_sent.id {
                info!(
                    "Updating inmempool transaction {} with new gas values for relayer: {}",
                    transaction_sent.id, self.relayer.name
                );
                transaction.known_transaction_hash = Some(transaction_sent.hash);
                transaction.sent_with_max_fee_per_gas =
                    Some(transaction_sent.sent_with_gas.max_fee);
                transaction.sent_with_max_priority_fee_per_gas =
                    Some(transaction_sent.sent_with_gas.max_priority_fee);
                transaction.sent_with_gas = Some(transaction_sent.sent_with_gas.clone());
                transaction.sent_at = Some(Utc::now());
            }
        }
    }

    pub async fn update_inmempool_transaction_noop(
        &mut self,
        transaction_id: &TransactionId,
        transaction_sent: &TransactionSentWithRelayer,
    ) {
        let mut transactions = self.inmempool_transactions.lock().await;
        if let Some(transaction) = transactions.front_mut() {
            if transaction.id == *transaction_id {
                info!(
                    "Updating inmempool transaction {} with no-op details for relayer: {}",
                    transaction_id, self.relayer.name
                );
                transaction.known_transaction_hash = Some(transaction_sent.hash);
                transaction.to = self.relay_address();
                transaction.value = TransactionValue::zero();
                transaction.data = TransactionData::empty();
                transaction.is_noop = true;
                transaction.speed = TransactionSpeed::Fast;
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
        if let Some(transaction) = transactions.front_mut() {
            if transaction.id == *transaction_id {
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
                transaction.sent_with_blob_gas = replacement_transaction.sent_with_blob_gas.clone();
                transaction.speed = replacement_transaction.speed.clone();
                transaction.sent_with_max_fee_per_gas =
                    replacement_transaction.sent_with_max_fee_per_gas;
                transaction.sent_with_max_priority_fee_per_gas =
                    transaction.sent_with_max_priority_fee_per_gas;
                transaction.is_noop = replacement_transaction.is_noop;
                transaction.external_id = replacement_transaction.external_id.clone();
            }
        }
    }

    pub async fn move_inmempool_to_mining(
        &mut self,
        id: &TransactionId,
        receipt: &AnyTransactionReceipt,
    ) -> Result<TransactionStatus, MoveInmempoolTransactionToMinedError> {
        info!(
            "Moving transaction {} from inmempool to mined for relayer: {} with receipt status: {}",
            id,
            self.relayer.name,
            receipt.status()
        );

        let mut transactions = self.inmempool_transactions.lock().await;
        let item = transactions.front().cloned();

        if let Some(transaction) = item {
            if transaction.id == *id {
                let transaction_status: TransactionStatus;

                if receipt.status() {
                    transaction_status = TransactionStatus::Mined;
                    info!(
                        "Transaction {} successfully mined for relayer: {}",
                        id, self.relayer.name
                    );
                } else {
                    transaction_status = TransactionStatus::Failed;
                    info!("Transaction {} failed on-chain for relayer: {}", id, self.relayer.name);
                }

                let mut mining_transactions = self.mined_transactions.lock().await;
                mining_transactions.insert(
                    transaction.id,
                    Transaction {
                        status: transaction_status.clone(),
                        mined_at: Some(Utc::now()),
                        ..transaction
                    },
                );

                transactions.pop_front();
                info!("Successfully moved transaction {} to mined status for relayer: {}. Inmempool: {}, Mined: {}",
                    id, self.relayer.name, transactions.len(), mining_transactions.len());

                Ok(transaction_status)
            } else {
                info!("Transaction ID mismatch when moving to mined for relayer: {}. Expected: {}, Found: {}",
                    self.relayer.name, id, transaction.id);
                Err(MoveInmempoolTransactionToMinedError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    *id,
                    transaction.clone(),
                ))
            }
        } else {
            info!(
                "No inmempool transaction found to move to mined for relayer: {} (transaction: {})",
                self.relayer.name, id
            );
            Err(MoveInmempoolTransactionToMinedError::TransactionNotFound(self.relayer.id, *id))
        }
    }

    pub async fn get_next_mined_transaction(&self) -> Option<Transaction> {
        let transactions = self.mined_transactions.lock().await;

        if let Some((_, value)) = transactions.iter().next() {
            return Some(value.clone());
        }

        None
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

    pub fn is_allowlisted_only(&self) -> bool {
        self.relayer.allowlisted_only
    }

    pub fn set_is_allowlisted_only(&mut self, is_allowlisted_only: bool) {
        info!(
            "Setting allowlisted only to {} for relayer: {}",
            is_allowlisted_only, self.relayer.name
        );
        self.relayer.allowlisted_only = is_allowlisted_only;
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

    pub async fn compute_gas_price_for_transaction(
        &self,
        transaction_speed: &TransactionSpeed,
        sent_last_with: Option<&GasPriceResult>,
    ) -> Result<GasPriceResult, SendTransactionGasPriceError> {
        info!(
            "Computing gas price for transaction with speed {:?} for relayer: {}",
            transaction_speed, self.relayer.name
        );

        let gas_oracle = self.gas_oracle_cache.lock().await;
        let mut gas_price = gas_oracle
            .get_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
            .await
            .ok_or(SendTransactionGasPriceError::GasCalculationError)?;

        if let Some(sent_gas) = sent_last_with {
            info!("Adjusting gas price based on previous attempt for relayer: {}. Previous max_fee: {}, max_priority_fee: {}",
                self.relayer.name, sent_gas.max_fee.into_u128(), sent_gas.max_priority_fee.into_u128());

            if gas_price.max_fee <= sent_gas.max_fee {
                let old_max_fee = gas_price.max_fee;
                gas_price.max_fee = sent_gas.max_fee + (sent_gas.max_fee / 10);
                info!(
                    "Bumped max_fee for relayer: {} from {} to {}",
                    self.relayer.name,
                    old_max_fee.into_u128(),
                    gas_price.max_fee.into_u128()
                );
            }

            if gas_price.max_priority_fee <= sent_gas.max_priority_fee {
                let old_priority_fee = gas_price.max_priority_fee;
                gas_price.max_priority_fee =
                    sent_gas.max_priority_fee + (sent_gas.max_priority_fee / 10);
                info!(
                    "Bumped max_priority_fee for relayer: {} from {} to {}",
                    self.relayer.name,
                    old_priority_fee.into_u128(),
                    gas_price.max_priority_fee.into_u128()
                );
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

        let blob_gas_oracle = self.blob_oracle_cache.lock().await;
        let mut blob_gas_price = blob_gas_oracle
            .get_blob_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
            .await
            .ok_or(SendTransactionGasPriceError::BlobGasCalculationError)?;

        if let Some(sent_blob_gas) = sent_last_with {
            info!("Adjusting blob gas price based on previous attempt for relayer: {}. Previous blob_gas_price: {}",
                self.relayer.name, sent_blob_gas.blob_gas_price);

            if blob_gas_price.blob_gas_price < sent_blob_gas.blob_gas_price {
                let old_blob_gas_price = blob_gas_price.blob_gas_price;
                blob_gas_price.blob_gas_price =
                    sent_blob_gas.blob_gas_price + (sent_blob_gas.blob_gas_price / 10);
                blob_gas_price.total_fee_for_blob =
                    blob_gas_price.blob_gas_price * BLOB_GAS_PER_BLOB;

                info!(
                    "Bumped blob gas price for relayer: {} from {} to {}, total_fee: {}",
                    self.relayer.name,
                    old_blob_gas_price,
                    blob_gas_price.blob_gas_price,
                    blob_gas_price.total_fee_for_blob
                );
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

        let signature =
            self.evm_provider.sign_transaction(&self.relayer.wallet_index, transaction).await?;

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

        let tx_hash = TransactionHash::from_alloy_hash(&hash);
        info!("Computed transaction hash {} for relayer: {}", tx_hash, self.relayer.name);
        Ok(tx_hash)
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
            let estimated_gas = estimated_gas_result * 12 / 10;
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

    pub async fn send_transaction(
        &mut self,
        db: &mut PostgresClient,
        transaction: &mut Transaction,
    ) -> Result<TransactionSentWithRelayer, TransactionQueueSendTransactionError> {
        info!(
            "Preparing to send transaction {} for relayer: {} with speed {:?}",
            transaction.id, self.relayer.name, transaction.speed
        );

        info!("Sending transaction {:?} for relayer: {}", transaction, self.relayer.name);

        let gas_price = self
            .compute_gas_price_for_transaction(
                &transaction.speed,
                transaction.sent_with_gas.as_ref(),
            )
            .await?;

        if !self.within_gas_price_bounds(&gas_price) {
            info!(
                "Transaction {} rejected - gas price too high for relayer: {}",
                transaction.id, self.relayer.name
            );
            return Err(TransactionQueueSendTransactionError::GasPriceTooHigh);
        }

        let (final_to, final_data) = if let Some(ref safe_proxy_manager) = self.safe_proxy_manager {
            if let Some(safe_address) =
                safe_proxy_manager.get_safe_proxy_for_relayer(&self.relayer.address)
            {
                info!(
                    "Routing transaction {} through safe proxy {} for relayer: {}",
                    transaction.id, safe_address, self.relayer.name
                );

                // TODO: safe_nonce!!!!
                // Get the safe's current nonce (this would need to be implemented)
                // For now, using a placeholder - this should get the actual safe nonce
                let safe_nonce = alloy::primitives::U256::ZERO;

                let (safe_addr, safe_tx) = safe_proxy_manager
                    .wrap_transaction_for_safe(
                        &self.relayer.address,
                        transaction.to,
                        transaction.value.clone(),
                        transaction.data.clone(),
                        safe_nonce,
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?;

                let safe_tx_hash = safe_proxy_manager
                    .get_safe_transaction_hash(
                        &safe_addr,
                        &safe_tx,
                        self.evm_provider.chain_id.u64(),
                    )
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?;

                let hash_hex = format!("0x{}", hex::encode(safe_tx_hash));

                let signature =
                    self.evm_provider
                        .sign_text(&self.relayer.wallet_index, &hash_hex)
                        .await
                        .map_err(|e| {
                            TransactionQueueSendTransactionError::TransactionConversionError(
                                format!("Failed to sign safe transaction hash: {}", e),
                            )
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

                let safe_call_data = safe_proxy_manager
                    .encode_safe_transaction(&safe_tx, signatures)
                    .map_err(|e| {
                        TransactionQueueSendTransactionError::TransactionConversionError(
                            e.to_string(),
                        )
                    })?;

                (safe_addr, TransactionData::new(safe_call_data))
            } else {
                (transaction.to, transaction.data.clone())
            }
        } else {
            (transaction.to, transaction.data.clone())
        };

        let mut working_transaction = transaction.clone();
        working_transaction.to = final_to;
        working_transaction.data = final_data;

        // If using safe proxy, the transaction value should be 0 because the ETH transfer
        // amount is encoded in the execTransaction call data, not in the transaction value
        if self.safe_proxy_manager.is_some()
            && self
                .safe_proxy_manager
                .as_ref()
                .unwrap()
                .get_safe_proxy_for_relayer(&self.relayer.address)
                .is_some()
        {
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

        // TODO: look at this for replacement and cancels
        let mut estimated_gas_limit = if let Some(gas_limit) = transaction.gas_limit {
            gas_limit
        } else {
            self.estimate_gas(&temp_transaction_request, working_transaction.is_noop)
                .await
                .map_err(TransactionQueueSendTransactionError::TransactionEstimateGasError)?
        };

        if self.safe_proxy_manager.is_some()
            && self
                .safe_proxy_manager
                .as_ref()
                .unwrap()
                .get_safe_proxy_for_relayer(&self.relayer.address)
                .is_some()
        {
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

        working_transaction.gas_limit = Some(estimated_gas_limit);
        transaction.gas_limit = Some(estimated_gas_limit);

        let transaction_request: TypedTransaction = if working_transaction.is_blob_transaction() {
            info!("Creating final blob transaction for relayer: {}", self.relayer.name);
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
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        } else if self.is_legacy_transactions() {
            info!("Creating final legacy transaction for relayer: {}", self.relayer.name);
            working_transaction
                .to_legacy_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        } else {
            info!("Creating final EIP-1559 transaction for relayer: {}", self.relayer.name);
            working_transaction
                .to_eip1559_typed_transaction_with_gas_limit(
                    Some(&gas_price),
                    Some(estimated_gas_limit),
                )
                .map_err(|e| {
                    TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
                })?
        };
        info!(
            "Set gas limit {} for transaction {} on relayer: {}",
            estimated_gas_limit.into_inner(),
            transaction.id,
            self.relayer.name
        );

        info!(
            "Sending transaction {:?} to network for relayer: {}",
            transaction_request, self.relayer.name
        );
        let transaction_hash = self
            .evm_provider
            .send_transaction(&self.relayer.wallet_index, transaction_request)
            .await
            .map_err(TransactionQueueSendTransactionError::TransactionSendError)?;

        let transaction_sent = TransactionSentWithRelayer {
            id: transaction.id,
            hash: transaction_hash,
            sent_with_gas: gas_price,
        };

        info!(
            "Transaction {} sent successfully with hash {} for relayer: {}",
            transaction_sent.id, transaction_sent.hash, self.relayer.name
        );

        if transaction.sent_with_gas.is_none() || transaction.is_noop {
            info!(
                "Updating database for sent transaction {} on relayer: {}",
                transaction.id, self.relayer.name
            );
            if transaction.sent_with_gas.is_none() {
                db.transaction_sent(
                    &transaction_sent.id,
                    &transaction_sent.hash,
                    &transaction_sent.sent_with_gas,
                    self.is_legacy_transactions(),
                )
                .await?;
            } else if transaction.is_noop {
                db.update_transaction_noop(&transaction.id, &transaction.to).await?;
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

    pub async fn get_balance(
        &self,
    ) -> Result<alloy::primitives::U256, RpcError<TransportErrorKind>> {
        let address = self.relay_address();
        self.evm_provider.get_balance(&address).await
    }
}
