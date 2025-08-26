use std::collections::{HashMap, VecDeque};

use crate::{
    provider::EvmProvider,
    relayer::types::Relayer,
    transaction::types::{Transaction, TransactionId},
};

/// Setup configuration for initializing a relayer's transaction queue.
///
/// Contains the relayer configuration, provider, and initial transaction states
/// loaded from the database during system startup.
pub struct TransactionRelayerSetup {
    pub relayer: Relayer,
    pub evm_provider: EvmProvider,
    pub pending_transactions: VecDeque<Transaction>,
    pub inmempool_transactions: VecDeque<Transaction>,
    pub mined_transactions: HashMap<TransactionId, Transaction>,
}

impl TransactionRelayerSetup {
    /// Creates a new transaction relayer setup configuration.
    ///
    /// # Arguments
    /// * `relayer` - The relayer configuration
    /// * `evm_provider` - The EVM provider for this relayer's network
    /// * `pending_transactions` - Transactions waiting to be sent
    /// * `inmempool_transactions` - Transactions sent but not yet mined
    /// * `mined_transactions` - Transactions mined but awaiting confirmations
    ///
    /// # Returns
    /// * `TransactionRelayerSetup` - The configured relayer setup
    pub fn new(
        relayer: Relayer,
        evm_provider: EvmProvider,
        pending_transactions: VecDeque<Transaction>,
        inmempool_transactions: VecDeque<Transaction>,
        mined_transactions: HashMap<TransactionId, Transaction>,
    ) -> Self {
        TransactionRelayerSetup {
            relayer,
            evm_provider,
            pending_transactions,
            inmempool_transactions,
            mined_transactions,
        }
    }
}
