use std::sync::Arc;

use tokio::sync::Mutex;

use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::shared::{unauthorized, HttpError};
use crate::transaction::types::TransactionValue;
use crate::yaml::{ApiKey, NetworkPermissionsConfig, NetworkSetupConfig};
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
use axum::http::HeaderMap;

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
    /// The API keys mapped to be able to be used
    pub api_keys: Arc<Vec<(ChainId, Vec<ApiKey>)>>,
    /// Network configurations to check feature availability
    pub network_configs: Arc<Vec<NetworkSetupConfig>>,
}

pub enum NetworkValidateAction {
    Transaction,
    SigningTypedData,
}

impl AppState {
    fn find_network_permission(
        &self,
        chain_id: &ChainId,
    ) -> Option<&Vec<NetworkPermissionsConfig>> {
        self.network_permissions.iter().find(|n| n.0 == *chain_id).map(|n| &n.1)
    }

    fn is_basic_auth_valid(&self, headers: &HeaderMap) -> bool {
        headers
            .get("x-rrelayer-basic-auth-valid")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true")
            .unwrap_or(false)
    }

    fn find_api_keys(&self, chain_id: &ChainId) -> Option<&Vec<ApiKey>> {
        self.api_keys.iter().find(|k| k.0 == *chain_id).map(|k| &k.1)
    }

    pub fn validate_basic_auth_valid(&self, headers: &HeaderMap) -> Result<(), HttpError> {
        if self.is_basic_auth_valid(headers) {
            Ok(())
        } else {
            Err(unauthorized(None))
        }
    }

    pub fn validate_allowed_passed_basic_auth(&self, headers: &HeaderMap) -> Result<(), HttpError> {
        let api_keys_enabled = self.api_keys.len() > 0;
        if !api_keys_enabled && !self.is_basic_auth_valid(headers) {
            return Err(unauthorized(None));
        }

        Ok(())
    }

    pub fn validate_auth_basic_or_api_key(
        &self,
        headers: &HeaderMap,
        relayer_address: &EvmAddress,
        chain_id: &ChainId,
    ) -> Result<(), HttpError> {
        if self.is_basic_auth_valid(headers) {
            return Ok(());
        }

        let passed = self.is_api_key_valid_for_relayer(headers, relayer_address, chain_id);
        if passed {
            Ok(())
        } else {
            Err(unauthorized(None))
        }
    }

    fn is_api_key_valid_for_relayer(
        &self,
        headers: &HeaderMap,
        relayer_address: &EvmAddress,
        chain_id: &ChainId,
    ) -> bool {
        let api_key = match headers.get("x-rrelayer-api-key").and_then(|v| v.to_str().ok()) {
            Some(key) => key,
            None => return false,
        };

        let api_keys = match self.find_api_keys(chain_id) {
            Some(keys) => keys,
            None => return false,
        };

        for api_key_config in api_keys {
            if api_key_config.relayer == *relayer_address {
                return api_key_config.keys.contains(&api_key.to_string());
            }
        }

        false
    }

    pub fn validate_api_key_and_get_relayers(
        &self,
        headers: &HeaderMap,
        chain_id: &ChainId,
    ) -> Option<Vec<EvmAddress>> {
        let api_key = headers.get("x-rrelayer-api-key").and_then(|v| v.to_str().ok())?;

        let api_keys = self.find_api_keys(chain_id)?;

        let mut accessible_relayers = Vec::new();
        for api_key_config in api_keys {
            if api_key_config.keys.contains(&api_key.to_string()) {
                accessible_relayers.push(api_key_config.relayer);
            }
        }

        if accessible_relayers.is_empty() {
            None
        } else {
            Some(accessible_relayers)
        }
    }

    pub fn is_api_key_valid_across_any_chain(&self, headers: &HeaderMap) -> bool {
        let api_key = match headers.get("x-rrelayer-api-key").and_then(|v| v.to_str().ok()) {
            Some(key) => key,
            None => return false,
        };

        for (_, api_keys) in &*self.api_keys {
            for api_key_config in api_keys {
                if api_key_config.keys.contains(&api_key.to_string()) {
                    return true;
                }
            }
        }

        false
    }

    pub fn get_api_key_access(
        &self,
        headers: &HeaderMap,
    ) -> Option<Vec<crate::authentication::api::ApiKeyAccess>> {
        let api_key = headers.get("x-rrelayer-api-key").and_then(|v| v.to_str().ok())?;

        let mut access_list = Vec::new();

        // Check all chains for this API key
        for (chain_id, api_keys) in &*self.api_keys {
            let mut relayers_for_chain = Vec::new();

            for api_key_config in api_keys {
                if api_key_config.keys.contains(&api_key.to_string()) {
                    relayers_for_chain.push(api_key_config.relayer);
                }
            }

            if !relayers_for_chain.is_empty() {
                access_list.push(crate::authentication::api::ApiKeyAccess {
                    chain_id: *chain_id,
                    relayers: relayers_for_chain,
                });
            }
        }

        if access_list.is_empty() {
            None
        } else {
            Some(access_list)
        }
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

    pub fn restricted_personal_signing(
        &self,
        relayer: &EvmAddress,
        chain_id: &ChainId,
    ) -> Result<(), HttpError> {
        let network_permission = self.find_network_permission(chain_id);
        if let Some(network_permissions) = network_permission {
            for network_permission in network_permissions {
                if network_permission.relayers.contains(&relayer) {
                    if network_permission.disable_personal_sign.unwrap_or_default() {
                        return Err(unauthorized(Some(
                            "Relayer have disabled personal signing".to_string(),
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn network_permission_validate(
        &self,
        relayer: &EvmAddress,
        chain_id: &ChainId,
        to: &EvmAddress,
        value: &TransactionValue,
        action: NetworkValidateAction,
    ) -> Result<(), HttpError> {
        let network_permissions = self.find_network_permission(chain_id);
        if let Some(network_permissions) = network_permissions {
            for network_permission in network_permissions {
                if network_permission.relayers.contains(&relayer) {
                    match action {
                        NetworkValidateAction::Transaction => {
                            if network_permission.disable_transactions.unwrap_or_default() {
                                return Err(unauthorized(Some(
                                    "Relayer have disabled transactions".to_string(),
                                )));
                            }
                        }
                        NetworkValidateAction::SigningTypedData => {
                            if network_permission.disable_typed_data_sign.unwrap_or_default() {
                                return Err(unauthorized(Some(
                                    "Relayer have disabled typed data signing".to_string(),
                                )));
                            }
                        }
                    }

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
