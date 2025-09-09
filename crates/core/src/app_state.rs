use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    gas::{blob_gas_oracle::BlobGasOracleCache, gas_oracle::GasOracleCache},
    postgres::PostgresClient,
    provider::EvmProvider,
    user_rate_limiting::UserRateLimiter,
    shared::cache::Cache,
    transaction::queue_system::transactions_queues::TransactionsQueues,
    webhooks::WebhookManager,
    yaml::RateLimitConfig,
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
/// - Rate limiting engine
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
    pub webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    /// Rate limiting engine
    pub user_rate_limiter: Option<Arc<UserRateLimiter>>,
    /// Rate limiting configuration
    pub rate_limit_config: Option<RateLimitConfig>,
}
