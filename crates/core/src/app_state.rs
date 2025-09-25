use std::sync::Arc;

use tokio::sync::Mutex;

use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::shared::{unauthorized, HttpError};
use crate::transaction::types::TransactionValue;
use crate::yaml::NetworkPermissionsConfig;
use crate::{
    gas::{BlobGasOracleCache, GasOracleCache},
    postgres::PostgresClient,
    provider::EvmProvider,
    rate_limiting::RateLimiter,
    shared::cache::Cache,
    transaction::queue_system::TransactionsQueues,
    webhooks::WebhookManager,
    yaml::RateLimitConfig,
    SafeProxyManager,
};

pub struct RelayersInternalOnly {
    values: Vec<(ChainId, EvmAddress)>,
}

impl RelayersInternalOnly {
    pub fn new(values: Vec<(ChainId, EvmAddress)>) -> Self {
        Self { values }
    }

    pub fn restricted(&self, relayer: &EvmAddress, chain_id: &ChainId) -> bool {
        self.values.iter().any(|(id, address)| id == chain_id && address == relayer)
    }
}

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
    /// The safe proxy manager can only change on startup
    pub safe_proxy_manager: Arc<SafeProxyManager>,
    /// Any relayers which can only be called by internal logic
    pub relayer_internal_only: Arc<RelayersInternalOnly>,
    /// Hold all networks permissions
    pub network_permissions: Arc<Vec<(ChainId, Vec<NetworkPermissionsConfig>)>>,
}

impl AppState {
    fn find_network_permission(
        &self,
        chain_id: &ChainId,
    ) -> Option<&Vec<NetworkPermissionsConfig>> {
        self.network_permissions.iter().find(|n| n.0 == *chain_id).map(|n| &n.1)
    }

    pub fn restricted_addresses(
        &self,
        relayer: &EvmAddress,
        chain_id: &ChainId,
    ) -> Vec<EvmAddress> {
        let mut addresses = vec![];
        let network_permission = self.find_network_permission(chain_id);
        if let Some(network_permissions) = network_permission {
            for network_permission in network_permissions {
                if network_permission.relayers.contains(&relayer) {
                    addresses.extend_from_slice(&network_permission.allowlist);
                }
            }
        }

        addresses
    }

    pub fn network_permission_validate(
        &self,
        relayer: &EvmAddress,
        chain_id: &ChainId,
        to: &EvmAddress,
        value: &TransactionValue,
    ) -> Result<(), HttpError> {
        let network_permissions = self.find_network_permission(chain_id);
        if let Some(network_permissions) = network_permissions {
            for network_permission in network_permissions {
                if network_permission.relayers.contains(&relayer) {
                    if network_permission.allowlist.len() > 0
                        && !network_permission.allowlist.contains(&to)
                    {
                        return Err(unauthorized(Some(
                            format!("relayer is not allowed to send transactions to {}", to)
                                .to_string(),
                        )));
                    }

                    if !value.is_zero()
                        && network_permission.disable_native_transfer.unwrap_or_default()
                    {
                        return Err(unauthorized(Some(
                            "native transfer is disabled for this relayer".to_string(),
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
