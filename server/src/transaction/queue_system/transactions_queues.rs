use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, SystemTime},
};

use alloy::{
    consensus::TypedTransaction,
    transports::{RpcError, TransportErrorKind},
};
use tokio::sync::Mutex;

use super::{
    transactions_queue::TransactionsQueue,
    types::{
        AddTransactionError, CancelTransactionError, EditableTransactionType,
        ProcessInmempoolStatus, ProcessInmempoolTransactionError, ProcessMinedStatus,
        ProcessMinedTransactionError, ProcessPendingStatus, ProcessPendingTransactionError,
        ProcessResult, ReplaceTransactionError, TransactionRelayerSetup, TransactionToSend,
        TransactionsQueueSetup,
    },
};
use crate::{
    gas::gas_oracle::GasOracleCache,
    postgres::PostgresClient,
    relayer::types::RelayerId,
    shared::{
        cache::Cache,
        common_types::{EvmAddress, WalletOrProviderError},
    },
    transaction::{
        api::RelayTransactionRequest,
        cache::invalidate_transaction_no_state_cache,
        nonce_manager::NonceManager,
        queue_system::types::TransactionQueueSendTransactionError,
        types::{Transaction, TransactionData, TransactionId, TransactionStatus, TransactionValue},
    },
};

pub struct TransactionsQueues {
    pub queues: Arc<Mutex<HashMap<RelayerId, TransactionsQueue>>>,
    pub relayer_block_times_ms: HashMap<RelayerId, u64>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    db: PostgresClient,
    cache: Arc<Cache>,
}

impl TransactionsQueues {
    pub async fn new(
        setups: Vec<TransactionRelayerSetup>,
        gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
        cache: Arc<Cache>,
    ) -> Result<Self, WalletOrProviderError> {
        let mut queues = HashMap::new();
        let mut relayer_block_times_ms = HashMap::new();

        for setup in setups {
            let current_nonce = setup.evm_provider.get_nonce(&setup.relayer.wallet_index).await?;

            relayer_block_times_ms.insert(setup.relayer.id, setup.evm_provider.blocks_every);

            queues.insert(
                setup.relayer.id,
                TransactionsQueue::new(
                    TransactionsQueueSetup::new(
                        setup.relayer,
                        setup.evm_provider,
                        NonceManager::new(current_nonce),
                        setup.pending_transactions,
                        setup.inmempool_transactions,
                        setup.mined_transactions,
                    ),
                    gas_oracle_cache.clone(),
                ),
            );
        }

        Ok(Self {
            queues: Arc::new(Mutex::new(queues)),
            relayer_block_times_ms,
            gas_oracle_cache,
            db: PostgresClient::new().await.expect("Failed to create PostgreSQL connection"),
            cache,
        })
    }

    async fn invalidate_transaction_cache(&self, id: &TransactionId) {
        invalidate_transaction_no_state_cache(&self.cache, id).await;
    }

    pub async fn pending_transactions_count(&mut self, relayer_id: &RelayerId) -> usize {
        if let Some(queue) = self.queues.lock().await.get_mut(relayer_id) {
            queue.get_pending_transaction_count().await
        } else {
            0
        }
    }

    pub async fn inmempool_transactions_count(&mut self, relayer_id: &RelayerId) -> usize {
        if let Some(queue) = self.queues.lock().await.get_mut(relayer_id) {
            queue.get_inmempool_transaction_count().await
        } else {
            0
        }
    }

    pub async fn add_new_relayer(
        &mut self,
        setup: TransactionsQueueSetup,
    ) -> Result<(), WalletOrProviderError> {
        let current_nonce = setup.evm_provider.get_nonce(&setup.relayer.wallet_index).await?;

        self.queues.lock().await.insert(
            setup.relayer.id,
            TransactionsQueue::new(
                TransactionsQueueSetup::new(
                    setup.relayer,
                    setup.evm_provider,
                    NonceManager::new(current_nonce),
                    VecDeque::new(),
                    VecDeque::new(),
                    HashMap::new(),
                ),
                self.gas_oracle_cache.clone(),
            ),
        );

        Ok(())
    }

    fn expires_at(&self) -> SystemTime {
        // 12 hours we then send them to noop
        SystemTime::now() + Duration::from_secs(12 * 60 * 60)
    }

    fn has_expired(&self, transaction: &Transaction) -> bool {
        transaction.expires_at < SystemTime::now()
    }

    fn transaction_to_noop(
        &self,
        transactions_queue: &mut TransactionsQueue,
        transaction: &mut Transaction,
    ) {
        transaction.to = transactions_queue.relay_address();
        transaction.value = TransactionValue::zero();
        transaction.data = TransactionData::empty();
        transaction.gas_limit = None;
        transaction.is_noop = true;
    }

    fn transaction_replace(
        &self,
        current_transaction: &mut Transaction,
        replace_with: &RelayTransactionRequest,
    ) {
        current_transaction.to = replace_with.to;
        current_transaction.data = replace_with.data.clone();
        current_transaction.value = replace_with.value;
        current_transaction.is_noop = current_transaction.from == current_transaction.to;
        current_transaction.gas_limit = None;
    }

    async fn relayer_allowed_to_send_transaction_to(
        &self,
        relayer_id: &RelayerId,
        to: &EvmAddress,
    ) -> Result<bool, tokio_postgres::Error> {
        let relayer = self.db.is_relayer_allowlist_address(relayer_id, to).await?;
        Ok(relayer)
    }

    pub async fn add_transaction(
        &mut self,
        relayer_id: &RelayerId,
        transaction_to_send: &TransactionToSend,
    ) -> Result<Transaction, AddTransactionError> {
        let expires_at = self.expires_at();

        if let Some(transactions_queue) = self.queues.lock().await.get_mut(relayer_id) {
            if transactions_queue.is_paused() {
                return Err(AddTransactionError::RelayerIsPaused(*relayer_id));
            }

            if transactions_queue.is_allowlisted_only() &&
                !self
                    .relayer_allowed_to_send_transaction_to(relayer_id, &transaction_to_send.to)
                    .await
                    .map_err(AddTransactionError::CouldNotReadAllowlistsFromDb)?
            {
                return Err(AddTransactionError::RelayerNotAllowedToSendTransactionTo(
                    *relayer_id,
                    transaction_to_send.to,
                ));
            }

            let mut transaction = Transaction {
                id: transaction_to_send.id,
                relayer_id: *relayer_id,
                to: transaction_to_send.to,
                from: transactions_queue.relay_address(),
                value: transaction_to_send.value,
                data: transaction_to_send.data.clone(),
                nonce: transactions_queue.nonce_manager.current(),
                // it works this out later
                gas_limit: None,
                status: TransactionStatus::Pending,
                chain_id: transactions_queue.chain_id(),
                known_transaction_hash: None,
                queued_at: SystemTime::now(),
                expires_at,
                sent_at: None,
                mined_at: None,
                speed: transaction_to_send.speed.clone(),
                sent_with_max_priority_fee_per_gas: None,
                sent_with_max_fee_per_gas: None,
                is_noop: false,
                from_api_key: transaction_to_send.from_api_key.clone(),
                sent_with_gas: None,
            };

            // have to add gas logic else we cant compute transaction hash
            // hash may change instantly with gas jumps but as it should be picked up
            // fast then gas most likely is the same
            let gas_price = transactions_queue
                .compute_gas_price_for_transaction(&transaction_to_send.speed, None)
                .await
                .map_err(AddTransactionError::TransactionGasPriceError)?;

            let transaction_request: TypedTransaction =
                if transactions_queue.is_legacy_transactions() {
                    transaction.to_legacy_typed_transaction(Some(&gas_price))
                } else {
                    transaction.to_eip1559_typed_transaction(Some(&gas_price))
                };

            let simulated = transactions_queue
                .simulate_transaction(&transaction_request)
                .await
                .map_err(|e| AddTransactionError::TransactionEstimateGasError(*relayer_id, e));

            if let Err(err) = simulated {
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

            // TODO! work out why hash never adds up
            transaction.known_transaction_hash = Some(
                transactions_queue
                    .compute_tx_hash(&transaction_request)
                    .await
                    .map_err(AddTransactionError::ComputeTransactionHashError)?,
            );

            transactions_queue.add_pending_transaction(transaction.clone()).await;

            self.db
                .save_transaction(relayer_id, &transaction)
                .await
                .map_err(AddTransactionError::CouldNotSaveTransactionDb)?;

            transactions_queue.nonce_manager.increase().await;

            self.invalidate_transaction_cache(&transaction.id).await;

            return Ok(transaction);
        }

        Err(AddTransactionError::RelayerNotFound(*relayer_id))
    }

    pub async fn cancel_transaction(
        &mut self,
        transaction: &Transaction,
    ) -> Result<bool, CancelTransactionError> {
        if let Some(transactions_queue) = self.queues.lock().await.get_mut(&transaction.relayer_id)
        {
            if transactions_queue.is_paused() {
                return Err(CancelTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if let Some(mut result) =
                transactions_queue.get_editable_transaction_by_id(&transaction.id).await
            {
                match result.type_name {
                    EditableTransactionType::Pending => {
                        self.transaction_to_noop(transactions_queue, &mut result.transaction);
                        self.invalidate_transaction_cache(&transaction.id).await;
                        Ok(true)
                    }
                    EditableTransactionType::Inmempool => {
                        self.transaction_to_noop(transactions_queue, &mut result.transaction);

                        transactions_queue
                            .send_transaction(&mut self.db, &mut result.transaction)
                            .await
                            .map_err(CancelTransactionError::SendTransactionError)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

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

    pub async fn replace_transaction(
        &mut self,
        transaction: &Transaction,
        replace_with: &RelayTransactionRequest,
    ) -> Result<bool, ReplaceTransactionError> {
        if let Some(transactions_queue) = self.queues.lock().await.get_mut(&transaction.relayer_id)
        {
            if transactions_queue.is_paused() {
                return Err(ReplaceTransactionError::RelayerIsPaused(transaction.relayer_id));
            }

            if transactions_queue.is_allowlisted_only() &&
                !self
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
                        self.transaction_replace(&mut result.transaction, replace_with);
                        self.invalidate_transaction_cache(&transaction.id).await;
                        Ok(true)
                    }
                    EditableTransactionType::Inmempool => {
                        self.transaction_replace(&mut result.transaction, replace_with);

                        transactions_queue
                            .send_transaction(&mut self.db, &mut result.transaction)
                            .await
                            .map_err(ReplaceTransactionError::SendTransactionError)?;

                        self.invalidate_transaction_cache(&transaction.id).await;

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

    pub async fn process_single_pending(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessPendingStatus>, ProcessPendingTransactionError> {
        let mut queue = self.queues.lock().await;
        let transactions_queue = queue.get_mut(relayer_id);

        match transactions_queue {
            Some(transactions_queue) => {
                if transactions_queue.is_paused() {
                    return Ok(ProcessResult::<ProcessPendingStatus>::other(
                        ProcessPendingStatus::RelayerPaused,
                        Some(&30000), // relayer paused we will wait 30 seconds to get new stuff
                    ));
                }

                if let Some(mut transaction) =
                    transactions_queue.get_next_pending_transaction().await
                {
                    if self.has_expired(&transaction) {
                        self.transaction_to_noop(transactions_queue, &mut transaction);
                    }

                    match transactions_queue.send_transaction(&mut self.db, &mut transaction).await
                    {
                        Ok(transaction_sent) => {
                            transactions_queue
                                .move_pending_to_inmempool(&transaction_sent)
                                .await.map_err( ProcessPendingTransactionError::MovePendingTransactionToInmempoolError)?;
                            self.invalidate_transaction_cache(&transaction.id).await;
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
                                TransactionQueueSendTransactionError::TransactionEstimateGasError(error) => {
                                    self.db
                                        .update_transaction_failed(
                                            &transaction.id,
                                            &error.to_string(),
                                        )
                                        .await
                                        .map_err(ProcessPendingTransactionError::DbError)?;

                                    transactions_queue.move_next_pending_to_failed().await;

                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    Err(
                                        ProcessPendingTransactionError::TransactionEstimateGasError(
                                            error,
                                        ),
                                    )
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
                                        TransactionQueueSendTransactionError::TransactionSendError(error),
                                    ))
                                }
                                TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(error) => {
                                    // just keep the transaction in pending state as could be a bad
                                    // db connection or temp
                                    // outage
                                    Err(ProcessPendingTransactionError::SendTransactionError(
                                        TransactionQueueSendTransactionError::CouldNotUpdateTransactionDb(error),
                                    ))
                                }
                                TransactionQueueSendTransactionError::SendTransactionGasPriceError(error) => {
                                    // should never happen if it does something internal is wrong,
                                    // and we don't want to
                                    // continue processing the queue
                                    // it can stay in a loop forever, so we don't fail pending
                                    // transactions
                                    Err(ProcessPendingTransactionError::SendTransactionError(
                                        TransactionQueueSendTransactionError::SendTransactionGasPriceError(error),
                                    ))
                                }
                            }
                        }
                    }

                    Ok(ProcessResult::<ProcessPendingStatus>::success())
                } else {
                    Ok(ProcessResult::<ProcessPendingStatus>::other(
                        ProcessPendingStatus::NoPendingTransactions,
                        Default::default(),
                    ))
                }
            }
            None => {
                Err(ProcessPendingTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
            }
        }
    }

    pub async fn process_single_inmempool(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessInmempoolStatus>, ProcessInmempoolTransactionError> {
        let mut queue = self.queues.lock().await;
        let transactions_queue = queue.get_mut(relayer_id);

        match transactions_queue {
            Some(transactions_queue) => {
                if let Some(mut transaction) =
                    transactions_queue.get_next_inmempool_transaction().await
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
                                            .transaction_mined(&transaction.id, &receipt)
                                            .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Mined, e))?;
                                        self.invalidate_transaction_cache(&transaction.id).await;
                                    }
                                    TransactionStatus::Expired => {
                                        self.db.transaction_expired(&transaction.id).await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Expired, e))?;
                                        self.invalidate_transaction_cache(&transaction.id).await;
                                    }
                                    TransactionStatus::Failed => {
                                        self.db
                                            .update_transaction_failed(&transaction.id, "Failed onchain")
                                            .await.map_err(|e| ProcessInmempoolTransactionError::CouldNotUpdateTransactionStatusInTheDatabase(*relayer_id, transaction.clone(), TransactionStatus::Failed, e))?;
                                        self.invalidate_transaction_cache(&transaction.id).await;
                                    }
                                    _ => {}
                                }

                                Ok(ProcessResult::<ProcessInmempoolStatus>::success())
                            }
                            Ok(None) => {
                                if transactions_queue.should_bump_gas(
                                    transaction.sent_at.unwrap().elapsed().unwrap().as_secs(),
                                    &transaction.speed,
                                ) {
                                    let transaction_sent = transactions_queue
                                        .send_transaction(&mut self.db, &mut transaction)
                                        .await
                                        .map_err(
                                            ProcessInmempoolTransactionError::SendTransactionError,
                                        )?;

                                    transaction.known_transaction_hash =
                                        Some(transaction_sent.hash);
                                    transaction.sent_with_max_fee_per_gas =
                                        Some(transaction_sent.sent_with_gas.max_fee);
                                    transaction.sent_with_max_priority_fee_per_gas =
                                        Some(transaction_sent.sent_with_gas.max_priority_fee);
                                    transaction.sent_with_gas =
                                        Some(transaction_sent.sent_with_gas.clone());
                                    transaction.sent_at = Some(SystemTime::now());

                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    return Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                        ProcessInmempoolStatus::GasIncreased,
                                        Default::default(),
                                    ));
                                }

                                Ok(ProcessResult::<ProcessInmempoolStatus>::other(
                                    ProcessInmempoolStatus::StillInmempool,
                                    Some(&500), // recheck again in 500ms
                                ))
                            }
                            Err(e) => Err(
                                ProcessInmempoolTransactionError::CouldNotGetTransactionReceipt(
                                    *relayer_id,
                                    transaction.clone(),
                                    e,
                                ),
                            ),
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
            }
            None => {
                Err(ProcessInmempoolTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
            }
        }
    }

    pub async fn process_single_mined(
        &mut self,
        relayer_id: &RelayerId,
    ) -> Result<ProcessResult<ProcessMinedStatus>, ProcessMinedTransactionError> {
        let mut queue = self.queues.lock().await;
        let transactions_queue = queue.get_mut(relayer_id);

        match transactions_queue {
            Some(transactions_queue) => {
                if let Some(transaction) = transactions_queue.get_next_mined_transaction().await {
                    if let Some(mined_at) = transaction.mined_at {
                        match mined_at.elapsed() {
                            Ok(elapsed) => {
                                if transactions_queue.in_confirmed_range(elapsed) {
                                    // check receipt still exists
                                    transactions_queue
                                        .get_receipt(&transaction.known_transaction_hash.unwrap())
                                        .await
                                        .map_err(|e| ProcessMinedTransactionError::CouldNotGetTransactionReceipt(*relayer_id, transaction.clone(), e))?
                                        .ok_or(ProcessMinedTransactionError::CouldNotGetTransactionReceipt(*relayer_id, transaction.clone(), RpcError::Transport(TransportErrorKind::Custom("No receipt".to_string().into()))))?;

                                    self.db
                                        .transaction_confirmed(&transaction.id)
                                        .await.map_err(|e| ProcessMinedTransactionError::TransactionConfirmedNotSaveToDatabase(*relayer_id, transaction.clone(), e))?;

                                    transactions_queue
                                        .move_mining_to_confirmed(&transaction.id)
                                        .await;

                                    self.invalidate_transaction_cache(&transaction.id).await;

                                    return Ok(ProcessResult::<ProcessMinedStatus>::success());
                                }

                                Ok(ProcessResult::<ProcessMinedStatus>::other(
                                    ProcessMinedStatus::NotConfirmedYet,
                                    Default::default(),
                                ))
                            }
                            Err(e) => Err(ProcessMinedTransactionError::MinedAtTimeError(
                                *relayer_id,
                                transaction.clone(),
                                e,
                            )),
                        }
                    } else {
                        Err(ProcessMinedTransactionError::NoMinedAt(
                            *relayer_id,
                            transaction.clone(),
                        ))
                    }
                } else {
                    Ok(ProcessResult::<ProcessMinedStatus>::other(
                        ProcessMinedStatus::NoMinedTransactions,
                        Default::default(),
                    ))
                }
            }
            None => {
                Err(ProcessMinedTransactionError::RelayerTransactionsQueueNotFound(*relayer_id))
            }
        }
    }
}
