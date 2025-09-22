use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    gas::{BlobGasOracleCache, GasOracleCache},
    postgres::PostgresClient,
    provider::EvmProvider,
    rate_limiting::RateLimiter,
    shared::cache::Cache,
    transaction::queue_system::TransactionsQueues,
    webhooks::WebhookManager,
    yaml::RateLimitConfig,
};

/// Global application state shared across all HTTP handlers.
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
    pub user_rate_limiter: Option<Arc<RateLimiter>>,
    /// Rate limiting configuration
    pub rate_limit_config: Option<RateLimitConfig>,
    /// Mutex to prevent concurrent relayer creation deadlocks
    pub relayer_creation_mutex: Arc<Mutex<()>>,
}
