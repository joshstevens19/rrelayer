use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    gas::{blob_gas_oracle::BlobGasOracleCache, gas_oracle::GasOracleCache},
    postgres::PostgresClient,
    provider::EvmProvider,
    shared::cache::Cache,
    transaction::queue_system::transactions_queues::TransactionsQueues,
    webhooks::WebhookManager,
};

/// Global application state shared across all HTTP handlers.
///
/// Contains all the shared resources needed by the RRelayer API:
/// - Database connections
/// - Blockchain provider connections
/// - Gas estimation caches
/// - Transaction processing queues
/// - General purpose cache
/// - Webhook delivery manager
///
/// All fields are wrapped in Arc for efficient cloning across threads,
/// with Mutex protection for mutable state.
pub struct AppState {
    /// Database client with connection pooling
    pub db: Arc<PostgresClient>,
    /// EVM blockchain provider connections
    pub evm_providers: Arc<Vec<EvmProvider>>,
    /// Cache for gas price estimations
    pub gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    /// Cache for blob gas price estimations (EIP-4844)
    pub blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    /// Transaction processing queues per network
    pub transactions_queues: Arc<Mutex<TransactionsQueues>>,
    /// General purpose caching layer
    pub cache: Arc<Cache>,
    /// Webhook delivery management
    pub webhook_manager: Arc<Mutex<WebhookManager>>,
}
