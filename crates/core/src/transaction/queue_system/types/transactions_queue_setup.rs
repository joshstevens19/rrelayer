use super::CompetitiveTransaction;
use crate::{
    provider::EvmProvider,
    relayer::Relayer,
    safe_proxy::SafeProxyManager,
    transaction::{
        nonce_manager::NonceManager,
        types::{Transaction, TransactionId},
    },
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

pub struct TransactionsQueueSetup {
    pub relayer: Relayer,
    pub evm_provider: EvmProvider,
    pub nonce_manager: NonceManager,
    pub pending_transactions: VecDeque<Transaction>,
    pub inmempool_transactions: VecDeque<CompetitiveTransaction>,
    pub mined_transactions: HashMap<TransactionId, Transaction>,
    pub safe_proxy_manager: Arc<SafeProxyManager>,
}

impl TransactionsQueueSetup {
    pub fn new(
        relayer: Relayer,
        evm_provider: EvmProvider,
        nonce_manager: NonceManager,
        pending_transactions: VecDeque<Transaction>,
        inmempool_transactions: VecDeque<CompetitiveTransaction>,
        mined_transactions: HashMap<TransactionId, Transaction>,
        safe_proxy_manager: Arc<SafeProxyManager>,
    ) -> Self {
        TransactionsQueueSetup {
            relayer,
            evm_provider,
            nonce_manager,
            pending_transactions,
            inmempool_transactions,
            mined_transactions,
            safe_proxy_manager,
        }
    }
}
