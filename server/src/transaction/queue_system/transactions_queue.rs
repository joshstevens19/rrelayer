use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use alloy::{
    consensus::{SignableTransaction, TypedTransaction},
    rpc::types::TransactionReceipt,
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
    gas::{fee_estimator::base::GasPriceResult, gas_oracle::GasOracleCache, types::GasLimit},
    network::types::ChainId,
    postgres::PostgresClient,
    provider::EvmProvider,
    relayer::types::Relayer,
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
    confirmations: u64,
}

impl TransactionsQueue {
    pub fn new(
        setup: TransactionsQueueSetup,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    ) -> Self {
        Self {
            pending_transactions: Mutex::new(setup.pending_transactions),
            inmempool_transactions: Mutex::new(setup.inmempool_transactions),
            mined_transactions: Mutex::new(setup.mined_transactions),
            evm_provider: setup.evm_provider,
            relayer: setup.relayer,
            nonce_manager: setup.nonce_manager,
            gas_oracle_cache,
            confirmations: 12,
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
        ms_between_times > (self.evm_provider.blocks_every * self.blocks_to_wait_before_bump(speed))
    }

    pub async fn add_pending_transaction(&mut self, transaction: Transaction) {
        let mut transactions = self.pending_transactions.lock().await;

        transactions.push_back(transaction);
    }

    pub async fn get_next_pending_transaction(&self) -> Option<Transaction> {
        let transactions = self.pending_transactions.lock().await;

        transactions.front().cloned()
    }

    pub async fn get_pending_transaction_count(&self) -> usize {
        let transactions = self.pending_transactions.lock().await;

        transactions.len()
    }

    pub async fn get_editable_transaction_by_id(
        &self,
        id: &TransactionId,
    ) -> Option<EditableTransaction> {
        let transactions = self.pending_transactions.lock().await;

        let pending = transactions.iter().find(|t| t.id == *id);

        match pending {
            Some(transaction) => Some(EditableTransaction::to_pending(transaction.clone())),
            None => {
                let transactions = self.inmempool_transactions.lock().await;
                transactions
                    .iter()
                    .find(|t| t.id == *id)
                    .map(|transaction| EditableTransaction::to_inmempool(transaction.clone()))
            }
        }
    }

    pub async fn move_pending_to_inmempool(
        &mut self,
        transaction_sent: &TransactionSentWithRelayer,
    ) -> Result<(), MovePendingTransactionToInmempoolError> {
        let mut transactions = self.pending_transactions.lock().await;

        let item = transactions.front().cloned();

        // extra checks just in-case should be impossible
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
                Ok(())
            } else {
                Err(MovePendingTransactionToInmempoolError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    transaction_sent.clone(),
                    transaction.clone(),
                ))
            }
        } else {
            Err(MovePendingTransactionToInmempoolError::TransactionNotFound(
                self.relayer.id,
                transaction_sent.clone(),
            ))
        }
    }

    pub async fn move_next_pending_to_failed(&mut self) {
        let mut transactions = self.pending_transactions.lock().await;

        // should always be the first item in reality as it processes one at a time
        transactions.pop_front();
    }

    pub async fn get_next_inmempool_transaction(&self) -> Option<Transaction> {
        let transactions = self.inmempool_transactions.lock().await;

        transactions.front().cloned()
    }

    pub async fn get_inmempool_transaction_count(&self) -> usize {
        let transactions = self.inmempool_transactions.lock().await;

        transactions.len()
    }

    pub async fn move_inmempool_to_mining(
        &mut self,
        id: &TransactionId,
        receipt: &TransactionReceipt,
    ) -> Result<TransactionStatus, MoveInmempoolTransactionToMinedError> {
        let mut transactions = self.inmempool_transactions.lock().await;

        let item = transactions.front().cloned();

        // extra checks just in case should be impossible
        if let Some(transaction) = item {
            if transaction.id == *id {
                let transaction_status: TransactionStatus;

                if receipt.status() {
                    if transaction.is_noop {
                        transaction_status = TransactionStatus::Expired;
                    } else {
                        transaction_status = TransactionStatus::Mined;
                    }
                } else {
                    transaction_status = TransactionStatus::Failed;
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

                Ok(transaction_status)
            } else {
                Err(MoveInmempoolTransactionToMinedError::TransactionIdDoesNotMatch(
                    self.relayer.id,
                    *id,
                    transaction.clone(),
                ))
            }
        } else {
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
        let mut transactions = self.mined_transactions.lock().await;

        transactions.remove(id);
    }

    pub fn relay_address(&self) -> EvmAddress {
        self.relayer.address
    }

    pub fn is_legacy_transactions(&self) -> bool {
        !self.relayer.eip_1559_enabled
    }

    pub fn is_allowlisted_only(&self) -> bool {
        self.relayer.allowlisted_only
    }

    pub fn is_paused(&self) -> bool {
        self.relayer.paused
    }

    pub fn chain_id(&self) -> ChainId {
        self.relayer.chain_id
    }

    fn within_gas_price_bounds(&self, gas: &GasPriceResult) -> bool {
        if let Some(max) = &self.relayer.max_gas_price {
            if self.relayer.eip_1559_enabled {
                return max.0 >= gas.max_fee.0
            }

            return max.0 >= gas.legacy_gas_price().0;
        }

        true
    }

    pub fn blocks_every_ms(&self) -> u64 {
        self.evm_provider.blocks_every
    }

    pub fn in_confirmed_range(&self, elapsed: Duration) -> bool {
        elapsed.as_secs() > (self.blocks_every_ms() * self.confirmations)
    }

    pub async fn compute_gas_price_for_transaction(
        &self,
        transaction_speed: &TransactionSpeed,
        sent_last_with: Option<&GasPriceResult>,
        //transaction: &Transaction,
    ) -> Result<GasPriceResult, SendTransactionGasPriceError> {
        let gas_oracle = self.gas_oracle_cache.lock().await;
        let mut gas_price = gas_oracle
            .get_gas_price_for_speed(&self.relayer.chain_id, transaction_speed)
            .await
            .ok_or(SendTransactionGasPriceError::GasCalculationError)?;

        if let Some(sent_gas) = sent_last_with {
            // Check if the oracle's max fee is lower than the transaction's max fee
            if gas_price.max_fee < sent_gas.max_fee {
                // If so, set the gas price's max fee to 10% higher than the transaction's max fee
                gas_price.max_fee = sent_gas.max_fee + (sent_gas.max_fee / 10);
            }

            // Similarly, check and adjust the max priority fee
            if gas_price.max_priority_fee < sent_gas.max_priority_fee {
                gas_price.max_priority_fee =
                    sent_gas.max_priority_fee + (sent_gas.max_priority_fee / 10);
            }
        }

        if let Some(sent_gas) = sent_last_with {
            // Check if the oracle's max fee is lower than the transaction's max fee
            if gas_price.max_fee < sent_gas.max_fee {
                // If so, set the gas price's max fee to 10% higher than the transaction's max fee
                gas_price.max_fee = sent_gas.max_fee + (sent_gas.max_fee / 10);
            }

            // Similarly, check and adjust the max priority fee
            if gas_price.max_priority_fee < sent_gas.max_priority_fee {
                gas_price.max_priority_fee =
                    sent_gas.max_priority_fee + (sent_gas.max_priority_fee / 10);
            }
        }

        Ok(gas_price)
    }

    pub async fn compute_tx_hash(
        &self,
        transaction: &TypedTransaction,
    ) -> Result<TransactionHash, LocalSignerError> {
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

        Ok(TransactionHash::from_alloy_hash(&hash))
    }

    pub async fn simulate_transaction(
        &self,
        transaction_request: &TypedTransaction,
    ) -> Result<(), RpcError<TransportErrorKind>> {
        // we just use the estimate gas for now as it's a good enough simulation
        self.estimate_gas(transaction_request, Default::default()).await?;

        Ok(())
    }

    pub async fn estimate_gas(
        &self,
        transaction_request: &TypedTransaction,
        is_noop: bool,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        // work out estimated gas for failing tx
        let estimated_gas_result = self.evm_provider.estimate_gas(transaction_request).await?;

        // only increase gas on contract logic
        if !is_noop {
            // Increase the estimated gas by 20%
            let estimated_gas = estimated_gas_result * 12 / 10;
            return Ok(estimated_gas);
        }

        Ok(estimated_gas_result)
    }

    pub async fn send_transaction(
        &mut self,
        db: &mut PostgresClient,
        transaction: &mut Transaction,
    ) -> Result<TransactionSentWithRelayer, TransactionQueueSendTransactionError> {
        let gas_price = self
            .compute_gas_price_for_transaction(
                &transaction.speed,
                transaction.sent_with_gas.as_ref(),
            )
            .await
            .map_err(TransactionQueueSendTransactionError::SendTransactionGasPriceError)?;

        if !self.within_gas_price_bounds(&gas_price) {
            return Err(TransactionQueueSendTransactionError::GasPriceTooHigh);
        }

        let transaction_request: TypedTransaction = if self.is_legacy_transactions() {
            transaction.to_legacy_typed_transaction(Some(&gas_price))
        } else {
            transaction.to_eip1559_typed_transaction(Some(&gas_price))
        };

        // work out estimated gas for failing tx to avoid state changing due to queue processing
        // we work this out as close to the send as possible
        let estimated_gas_limit = self
            .estimate_gas(&transaction_request, transaction.is_noop)
            .await
            .map_err(TransactionQueueSendTransactionError::TransactionEstimateGasError)?;

        transaction.gas_limit = Some(estimated_gas_limit);

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

        db.transaction_sent(
            &transaction_sent.id,
            &transaction_sent.hash,
            &transaction_sent.sent_with_gas,
            self.is_legacy_transactions(),
        )
        .await
        .map_err(TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb)?;

        Ok(transaction_sent)
    }

    pub async fn get_receipt(
        &mut self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<TransactionReceipt>, RpcError<TransportErrorKind>> {
        self.evm_provider.get_receipt(transaction_hash).await
    }
}
