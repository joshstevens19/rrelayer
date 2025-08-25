use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use alloy::network::{AnyTransactionReceipt, ReceiptResponse};
use alloy::{
    consensus::{SignableTransaction, TypedTransaction},
    signers::local::LocalSignerError,
    transports::{RpcError, TransportErrorKind},
};
use tokio::sync::Mutex;

use super::types::{
    EditableTransaction, MoveInmempoolTransactionToMinedError,
    MovePendingTransactionToInmempoolError, SendTransactionGasPriceError,
    TransactionQueueSendTransactionError, TransactionSentWithRelayer, TransactionsQueueSetup,
};
use crate::{
    gas::{
        blob_gas_oracle::{BlobGasOracleCache, BlobGasPriceResult, BLOB_GAS_PER_BLOB},
        fee_estimator::base::GasPriceResult,
        gas_oracle::GasOracleCache,
        types::{GasLimit, GasPrice},
    },
    network::types::ChainId,
    postgres::PostgresClient,
    provider::EvmProvider,
    relayer::types::Relayer,
    rrelayer_info,
    shared::common_types::EvmAddress,
    transaction::{
        nonce_manager::NonceManager,
        types::{Transaction, TransactionHash, TransactionId, TransactionSpeed, TransactionStatus},
    },
};

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
}

impl TransactionsQueue {
    pub fn new(
        setup: TransactionsQueueSetup,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
        blob_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    ) -> Self {
        rrelayer_info!(
            "Creating new TransactionsQueue for relayer: {} (name: {}) on chain: {}",
            setup.relayer.id,
            setup.relayer.name,
            setup.relayer.chain_id
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
            rrelayer_info!(
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
        rrelayer_info!(
            "Adding pending transaction {} to queue for relayer: {}",
            transaction.id,
            self.relayer.name
        );
        let mut transactions = self.pending_transactions.lock().await;
        transactions.push_back(transaction);
        rrelayer_info!(
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
        rrelayer_info!(
            "Current pending transaction count for relayer {}: {}",
            self.relayer.name,
            count
        );
        count
    }

    pub async fn get_editable_transaction_by_id(
        &self,
        id: &TransactionId,
    ) -> Option<EditableTransaction> {
        rrelayer_info!(
            "Looking for editable transaction {} for relayer: {}",
            id,
            self.relayer.name
        );
        let transactions = self.pending_transactions.lock().await;

        let pending = transactions.iter().find(|t| t.id == *id);

        match pending {
            Some(transaction) => {
                rrelayer_info!(
                    "Found transaction {} in pending queue for relayer: {}",
                    id,
                    self.relayer.name
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
                    rrelayer_info!(
                        "Found transaction {} in inmempool queue for relayer: {}",
                        id,
                        self.relayer.name
                    );
                } else {
                    rrelayer_info!(
                        "Transaction {} not found in any queue for relayer: {}",
                        id,
                        self.relayer.name
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
        rrelayer_info!(
            "Moving transaction {} from pending to inmempool for relayer: {} with hash: {}",
            transaction_sent.id,
            self.relayer.name,
            transaction_sent.hash
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
                    sent_at: Some(SystemTime::now()),
                    ..transaction
                });

                transactions.pop_front();
                rrelayer_info!("Successfully moved transaction {} to inmempool for relayer: {}. Pending: {}, Inmempool: {}",
                    transaction_sent.id, self.relayer.name, transactions.len(), inmempool_transactions.len());
                Ok(())
            } else {
                rrelayer_info!("Transaction ID mismatch when moving to inmempool for relayer: {}. Expected: {}, Found: {}",
                    self.relayer.name, transaction_sent.id, transaction.id);
                Err(MovePendingTransactionToInmempoolError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    transaction_sent.clone(),
                    transaction.clone(),
                ))
            }
        } else {
            rrelayer_info!("No pending transaction found to move to inmempool for relayer: {} (transaction: {})",
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
            rrelayer_info!(
                "Moving pending transaction {} to failed for relayer: {}",
                tx.id,
                self.relayer.name
            );
        }
        transactions.pop_front();
        rrelayer_info!(
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
        rrelayer_info!(
            "Current inmempool transaction count for relayer {}: {}",
            self.relayer.name,
            count
        );
        count
    }

    pub async fn move_inmempool_to_mining(
        &mut self,
        id: &TransactionId,
        receipt: &AnyTransactionReceipt,
    ) -> Result<TransactionStatus, MoveInmempoolTransactionToMinedError> {
        rrelayer_info!(
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
                    if transaction.is_noop {
                        transaction_status = TransactionStatus::Expired;
                        rrelayer_info!(
                            "Transaction {} marked as expired (noop) for relayer: {}",
                            id,
                            self.relayer.name
                        );
                    } else {
                        transaction_status = TransactionStatus::Mined;
                        rrelayer_info!(
                            "Transaction {} successfully mined for relayer: {}",
                            id,
                            self.relayer.name
                        );
                    }
                } else {
                    transaction_status = TransactionStatus::Failed;
                    rrelayer_info!(
                        "Transaction {} failed on-chain for relayer: {}",
                        id,
                        self.relayer.name
                    );
                }

                let mut mining_transactions = self.mined_transactions.lock().await;
                mining_transactions.insert(
                    transaction.id,
                    Transaction {
                        status: transaction_status.clone(),
                        mined_at: Some(SystemTime::now()),
                        ..transaction
                    },
                );

                transactions.pop_front();
                rrelayer_info!("Successfully moved transaction {} to mined status for relayer: {}. Inmempool: {}, Mined: {}",
                    id, self.relayer.name, transactions.len(), mining_transactions.len());

                Ok(transaction_status)
            } else {
                rrelayer_info!("Transaction ID mismatch when moving to mined for relayer: {}. Expected: {}, Found: {}",
                    self.relayer.name, id, transaction.id);
                Err(MoveInmempoolTransactionToMinedError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    *id,
                    transaction.clone(),
                ))
            }
        } else {
            rrelayer_info!(
                "No inmempool transaction found to move to mined for relayer: {} (transaction: {})",
                self.relayer.name,
                id
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
        rrelayer_info!(
            "Moving transaction {} from mined to confirmed for relayer: {}",
            id,
            self.relayer.name
        );
        let mut transactions = self.mined_transactions.lock().await;
        transactions.remove(id);
        rrelayer_info!(
            "Successfully confirmed transaction {} for relayer: {}. Remaining mined: {}",
            id,
            self.relayer.name,
            transactions.len()
        );
    }

    pub fn relay_address(&self) -> EvmAddress {
        self.relayer.address
    }

    pub fn is_legacy_transactions(&self) -> bool {
        !self.relayer.eip_1559_enabled
    }

    pub fn set_is_legacy_transactions(&mut self, is_legacy_transactions: bool) {
        rrelayer_info!(
            "Setting legacy transactions to {} for relayer: {}",
            is_legacy_transactions,
            self.relayer.name
        );
        self.relayer.eip_1559_enabled = is_legacy_transactions;
    }

    pub fn is_allowlisted_only(&self) -> bool {
        self.relayer.allowlisted_only
    }

    pub fn set_is_allowlisted_only(&mut self, is_allowlisted_only: bool) {
        rrelayer_info!(
            "Setting allowlisted only to {} for relayer: {}",
            is_allowlisted_only,
            self.relayer.name
        );
        self.relayer.allowlisted_only = is_allowlisted_only;
    }

    pub fn is_paused(&self) -> bool {
        self.relayer.paused
    }

    pub fn set_is_paused(&mut self, is_paused: bool) {
        rrelayer_info!("Setting paused to {} for relayer: {}", is_paused, self.relayer.name);
        self.relayer.paused = is_paused;
    }

    pub fn set_name(&mut self, name: &str) {
        rrelayer_info!("Changing relayer name from {} to {}", self.relayer.name, name);
        self.relayer.name = name.to_string();
    }

    pub fn max_gas_price(&self) -> Option<GasPrice> {
        self.relayer.max_gas_price
    }

    pub fn set_max_gas_price(&mut self, max_gas_price: Option<GasPrice>) {
        rrelayer_info!(
            "Setting max gas price to {:?} for relayer: {}",
            max_gas_price,
            self.relayer.name
        );
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
                rrelayer_info!(
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

    pub fn in_confirmed_range(&self, elapsed: Duration) -> bool {
        let threshold = self.blocks_every_ms() * self.confirmations;
        let in_range = elapsed.as_secs() > threshold;
        if in_range {
            rrelayer_info!(
                "Transaction in confirmed range for relayer: {} - elapsed: {}s, threshold: {}s",
                self.relayer.name,
                elapsed.as_secs(),
                threshold
            );
        }
        in_range
    }

    pub async fn compute_gas_price_for_transaction(
        &self,
        transaction_speed: &TransactionSpeed,
        sent_last_with: Option<&GasPriceResult>,
    ) -> Result<GasPriceResult, SendTransactionGasPriceError> {
        rrelayer_info!(
            "Computing gas price for transaction with speed {:?} for relayer: {}",
            transaction_speed,
            self.relayer.name
        );

        let gas_oracle = self.gas_oracle_cache.lock().await;
        let mut gas_price = gas_oracle
            .get_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
            .await
            .ok_or(SendTransactionGasPriceError::GasCalculationError)?;

        if let Some(sent_gas) = sent_last_with {
            rrelayer_info!("Adjusting gas price based on previous attempt for relayer: {}. Previous max_fee: {}, max_priority_fee: {}",
                self.relayer.name, sent_gas.max_fee.into_u128(), sent_gas.max_priority_fee.into_u128());

            if gas_price.max_fee < sent_gas.max_fee {
                let old_max_fee = gas_price.max_fee;
                gas_price.max_fee = sent_gas.max_fee + (sent_gas.max_fee / 10);
                rrelayer_info!(
                    "Bumped max_fee for relayer: {} from {} to {}",
                    self.relayer.name,
                    old_max_fee.into_u128(),
                    gas_price.max_fee.into_u128()
                );
            }

            if gas_price.max_priority_fee < sent_gas.max_priority_fee {
                let old_priority_fee = gas_price.max_priority_fee;
                gas_price.max_priority_fee =
                    sent_gas.max_priority_fee + (sent_gas.max_priority_fee / 10);
                rrelayer_info!(
                    "Bumped max_priority_fee for relayer: {} from {} to {}",
                    self.relayer.name,
                    old_priority_fee.into_u128(),
                    gas_price.max_priority_fee.into_u128()
                );
            }
        }

        rrelayer_info!(
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
        rrelayer_info!(
            "Computing blob gas price for transaction with speed {:?} for relayer: {}",
            transaction_speed,
            self.relayer.name
        );

        let blob_gas_oracle = self.blob_oracle_cache.lock().await;
        let mut blob_gas_price = blob_gas_oracle
            .get_blob_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
            .await
            .ok_or(SendTransactionGasPriceError::BlobGasCalculationError)?;

        if let Some(sent_blob_gas) = sent_last_with {
            rrelayer_info!("Adjusting blob gas price based on previous attempt for relayer: {}. Previous blob_gas_price: {}",
                self.relayer.name, sent_blob_gas.blob_gas_price);

            if blob_gas_price.blob_gas_price < sent_blob_gas.blob_gas_price {
                let old_blob_gas_price = blob_gas_price.blob_gas_price;
                blob_gas_price.blob_gas_price =
                    sent_blob_gas.blob_gas_price + (sent_blob_gas.blob_gas_price / 10);
                blob_gas_price.total_fee_for_blob =
                    blob_gas_price.blob_gas_price * BLOB_GAS_PER_BLOB;

                rrelayer_info!(
                    "Bumped blob gas price for relayer: {} from {} to {}, total_fee: {}",
                    self.relayer.name,
                    old_blob_gas_price,
                    blob_gas_price.blob_gas_price,
                    blob_gas_price.total_fee_for_blob
                );
            }
        }

        rrelayer_info!(
            "Final blob gas price for relayer: {} - blob_gas_price: {}, total_fee: {}",
            self.relayer.name,
            blob_gas_price.blob_gas_price,
            blob_gas_price.total_fee_for_blob
        );

        Ok(blob_gas_price)
    }

    pub async fn compute_tx_hash(
        &self,
        transaction: &TypedTransaction,
    ) -> Result<TransactionHash, LocalSignerError> {
        rrelayer_info!("Computing transaction hash for relayer: {}", self.relayer.name);

        let signature = self
            .evm_provider
            .sign_transaction(&self.relayer.wallet_index, transaction)
            .await
            .unwrap(); // TODO: fix error

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
        rrelayer_info!("Computed transaction hash {} for relayer: {}", tx_hash, self.relayer.name);
        Ok(tx_hash)
    }

    pub async fn simulate_transaction(
        &self,
        transaction_request: &TypedTransaction,
    ) -> Result<(), RpcError<TransportErrorKind>> {
        rrelayer_info!("Simulating transaction for relayer: {}", self.relayer.name);
        self.estimate_gas(transaction_request, Default::default()).await?;
        rrelayer_info!("Transaction simulation successful for relayer: {}", self.relayer.name);
        Ok(())
    }

    pub async fn estimate_gas(
        &self,
        transaction_request: &TypedTransaction,
        is_noop: bool,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        rrelayer_info!(
            "Estimating gas for transaction (noop: {}) for relayer: {}",
            is_noop,
            self.relayer.name
        );

        let estimated_gas_result = self.evm_provider.estimate_gas(transaction_request).await?;

        if !is_noop {
            let estimated_gas = estimated_gas_result * 12 / 10;
            rrelayer_info!(
                "Gas estimation for relayer: {} - base: {}, with 20% buffer: {}",
                self.relayer.name,
                estimated_gas_result.into_inner(),
                estimated_gas.into_inner()
            );
            return Ok(estimated_gas);
        }

        rrelayer_info!(
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
        rrelayer_info!(
            "Preparing to send transaction {} for relayer: {} with speed {:?}",
            transaction.id,
            self.relayer.name,
            transaction.speed
        );

        let gas_price = self
            .compute_gas_price_for_transaction(
                &transaction.speed,
                transaction.sent_with_gas.as_ref(),
            )
            .await
            .map_err(TransactionQueueSendTransactionError::SendTransactionGasPriceError)?;

        if !self.within_gas_price_bounds(&gas_price) {
            rrelayer_info!(
                "Transaction {} rejected - gas price too high for relayer: {}",
                transaction.id,
                self.relayer.name
            );
            return Err(TransactionQueueSendTransactionError::GasPriceTooHigh);
        }

        let transaction_request: TypedTransaction = if transaction.is_blob_transaction() {
            rrelayer_info!("Creating blob transaction for relayer: {}", self.relayer.name);
            let blob_gas_price = self
                .compute_blob_gas_price_for_transaction(
                    &transaction.speed,
                    &transaction.sent_with_blob_gas,
                )
                .await?;
            transaction.to_blob_typed_transaction(Some(&gas_price), Some(&blob_gas_price)).map_err(
                |e| TransactionQueueSendTransactionError::TransactionConversionError(e.to_string()),
            )?
        } else if self.is_legacy_transactions() {
            rrelayer_info!("Creating legacy transaction for relayer: {}", self.relayer.name);
            transaction.to_legacy_typed_transaction(Some(&gas_price)).map_err(|e| {
                TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
            })?
        } else {
            rrelayer_info!("Creating EIP-1559 transaction for relayer: {}", self.relayer.name);
            transaction.to_eip1559_typed_transaction(Some(&gas_price)).map_err(|e| {
                TransactionQueueSendTransactionError::TransactionConversionError(e.to_string())
            })?
        };

        let estimated_gas_limit = self
            .estimate_gas(&transaction_request, transaction.is_noop)
            .await
            .map_err(TransactionQueueSendTransactionError::TransactionEstimateGasError)?;

        transaction.gas_limit = Some(estimated_gas_limit);
        rrelayer_info!(
            "Set gas limit {} for transaction {} on relayer: {}",
            estimated_gas_limit.into_inner(),
            transaction.id,
            self.relayer.name
        );

        rrelayer_info!(
            "Sending transaction {:?} to network for relayer: {}",
            transaction_request,
            self.relayer.name
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

        rrelayer_info!(
            "Transaction {} sent successfully with hash {} for relayer: {}",
            transaction_sent.id,
            transaction_sent.hash,
            self.relayer.name
        );

        rrelayer_info!(
            "Updating database for sent transaction {} on relayer: {}",
            transaction.id,
            self.relayer.name
        );
        db.transaction_sent(
            &transaction_sent.id,
            &transaction_sent.hash,
            &transaction_sent.sent_with_gas,
            self.is_legacy_transactions(),
        )
        .await
        .map_err(TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb)?;

        rrelayer_info!(
            "Successfully processed transaction {} for relayer: {}",
            transaction.id,
            self.relayer.name
        );
        Ok(transaction_sent)
    }

    pub async fn get_receipt(
        &mut self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<AnyTransactionReceipt>, RpcError<TransportErrorKind>> {
        rrelayer_info!(
            "Getting receipt for transaction hash {} on relayer: {}",
            transaction_hash,
            self.relayer.name
        );
        let receipt = self.evm_provider.get_receipt(transaction_hash).await?;

        if receipt.is_some() {
            rrelayer_info!(
                "Receipt found for transaction hash {} on relayer: {}",
                transaction_hash,
                self.relayer.name
            );
        } else {
            rrelayer_info!(
                "No receipt found for transaction hash {} on relayer: {}",
                transaction_hash,
                self.relayer.name
            );
        }

        Ok(receipt)
    }
}
