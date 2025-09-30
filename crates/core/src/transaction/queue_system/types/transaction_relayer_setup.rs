use std::collections::{HashMap, VecDeque};

use super::CompetitiveTransaction;
use crate::{
    provider::EvmProvider,
    relayer::Relayer,
    transaction::types::{Transaction, TransactionId},
    yaml::GasBumpBlockConfig,
};

pub struct TransactionRelayerSetup {
    pub relayer: Relayer,
    pub evm_provider: EvmProvider,
    pub pending_transactions: VecDeque<Transaction>,
    pub inmempool_transactions: VecDeque<CompetitiveTransaction>,
    pub mined_transactions: HashMap<TransactionId, Transaction>,
    pub gas_bump_config: GasBumpBlockConfig,
    pub max_gas_price_multiplier: u64,
}

impl TransactionRelayerSetup {
    pub fn new(
        relayer: Relayer,
        evm_provider: EvmProvider,
        pending_transactions: VecDeque<Transaction>,
        inmempool_transactions: VecDeque<CompetitiveTransaction>,
        mined_transactions: HashMap<TransactionId, Transaction>,
        gas_bump_config: GasBumpBlockConfig,
        max_gas_price_multiplier: u64,
    ) -> Self {
        TransactionRelayerSetup {
            relayer,
            evm_provider,
            pending_transactions,
            inmempool_transactions,
            mined_transactions,
            gas_bump_config,
            max_gas_price_multiplier,
        }
    }
}
