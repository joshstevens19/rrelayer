use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    gas::{blob_gas_oracle::BlobGasOracleCache, gas_oracle::GasOracleCache},
    postgres::PostgresClient,
    provider::EvmProvider,
    shared::cache::Cache,
    transaction::queue_system::transactions_queues::TransactionsQueues,
};

pub struct AppState {
    pub db: Arc<PostgresClient>,
    pub evm_providers: Arc<Vec<EvmProvider>>,
    pub gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    pub blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    pub transactions_queues: Arc<Mutex<TransactionsQueues>>,
    pub cache: Arc<Cache>,
}
