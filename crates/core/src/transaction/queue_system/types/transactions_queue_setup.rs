use super::CompetitiveTransaction;
use crate::{
    provider::EvmProvider,
    relayer::Relayer,
    safe_proxy::SafeProxyManager,
    transaction::{
        nonce_manager::NonceManager,
        types::{Transaction, TransactionId},
    },
    yaml::GasBumpBlockConfig,
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
    pub gas_bump_config: GasBumpBlockConfig,
    pub max_gas_price_multiplier: u64,
}

impl TransactionsQueueSetup {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        relayer: Relayer,
        evm_provider: EvmProvider,
        nonce_manager: NonceManager,
        pending_transactions: VecDeque<Transaction>,
        inmempool_transactions: VecDeque<CompetitiveTransaction>,
        mined_transactions: HashMap<TransactionId, Transaction>,
        safe_proxy_manager: Arc<SafeProxyManager>,
        gas_bump_config: GasBumpBlockConfig,
        max_gas_price_multiplier: u64,
    ) -> Self {
        TransactionsQueueSetup {
            relayer,
            evm_provider,
            nonce_manager,
            pending_transactions,
            inmempool_transactions,
            mined_transactions,
            safe_proxy_manager,
            gas_bump_config,
            max_gas_price_multiplier,
        }
    }
}
