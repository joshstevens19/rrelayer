use std::sync::Arc;

use crate::api::{
    ApiResult, HealthApi, NetworkApi, RelayerApi, SignApi, TransactionApi,
    http::HttpClient,
    types::{ApiBaseConfig, AuthConfig},
};
use crate::{ApiSdkError, AuthenticationApi};
use rrelayer_core::authentication::api::StatusResponse;
use rrelayer_core::signing::{SignedTextHistory, SignedTypedDataHistory};
use rrelayer_core::transaction::api::{
    CancelTransactionResponse, RelayTransactionRequest, SendTransactionResult,
};
use rrelayer_core::transaction::types::{TransactionSpeed, TransactionStatus};
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    gas::GasEstimatorResult,
    network::{ChainId, Network},
    relayer::{CreateRelayerResult, GetRelayerResult, Relayer, RelayerId},
    transaction::api::RelayTransactionStatusResult,
    transaction::types::{Transaction, TransactionId},
};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct CreateClientConfig {
    pub server_url: String,
    pub auth: CreateClientAuth,
}

#[derive(Debug, Clone)]
pub struct CreateClientAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct CreateRelayerClientConfig {
    pub server_url: String,
    pub relayer_id: RelayerId,
    pub api_key: String,
    pub speed: Option<TransactionSpeed>,
}

#[derive(Clone)]
pub struct Client {
    config: CreateClientConfig,
    api_base_config: ApiBaseConfig,
    authentication_api: AuthenticationApi,
    network_api: NetworkApi,
    relayer_api: RelayerApi,
    transaction_api: TransactionApi,
    health_api: HealthApi,
}

impl Client {
    pub fn new(config: CreateClientConfig) -> Self {
        let api_base_config = ApiBaseConfig {
            server_url: config.server_url.clone(),
            auth: AuthConfig::BasicAuth {
                username: config.auth.username.clone(),
                password: config.auth.password.clone(),
            },
        };
        let client = Arc::new(HttpClient::new(api_base_config.clone()));

        Self {
            config,
            api_base_config,
            authentication_api: AuthenticationApi::new(Arc::clone(&client)),
            network_api: NetworkApi::new(Arc::clone(&client)),
            relayer_api: RelayerApi::new(Arc::clone(&client)),
            transaction_api: TransactionApi::new(Arc::clone(&client)),
            health_api: HealthApi::new(Arc::clone(&client)),
        }
    }

    pub fn relayer(&self) -> ClientRelayerApi<'_> {
        ClientRelayerApi { relayer_api: &self.relayer_api }
    }

    pub fn network(&self) -> ClientNetworkApi<'_> {
        ClientNetworkApi { network_api: &self.network_api }
    }

    pub fn transaction(&self) -> ClientTransactionApi<'_> {
        ClientTransactionApi { transaction_api: &self.transaction_api }
    }

    pub fn allowlist(&self) -> ClientAllowlistApi<'_> {
        ClientAllowlistApi { relayer_api: &self.relayer_api }
    }

    pub async fn get_relayer_client(
        &self,
        relayer_id: &RelayerId,
        speed: Option<TransactionSpeed>,
    ) -> ApiResult<AdminRelayerClient> {
        let relayer = self.relayer_api.get(relayer_id).await?.ok_or_else(|| {
            ApiSdkError::ConfigError(format!("Relayer '{}' not found", relayer_id))
        })?;

        let network = self.network_api.get(&relayer.relayer.chain_id).await?.ok_or_else(|| {
            ApiSdkError::ConfigError(format!("Network '{}' not found", relayer.relayer.chain_id))
        })?;

        let provider_url = network
            .provider_urls
            .first()
            .ok_or_else(|| {
                ApiSdkError::ConfigError(format!(
                    "No provider URLs found for network '{}'",
                    relayer.relayer.chain_id
                ))
            })?
            .clone();

        Ok(AdminRelayerClient::new(AdminRelayerClientConfig {
            server_url: self.config.server_url.clone(),
            provider_url,
            relayer_id: *relayer_id,
            auth: AdminRelayerClientAuth::BasicAuth {
                username: self.config.auth.username.clone(),
                password: self.config.auth.password.clone(),
            },
            speed,
        }))
    }

    pub async fn health(&self) -> ApiResult<()> {
        self.health_api.check().await
    }

    pub async fn authenticated(&self) -> ApiResult<StatusResponse> {
        self.authentication_api.test_auth().await
    }
}

pub struct ClientRelayerApi<'a> {
    relayer_api: &'a RelayerApi,
}

impl<'a> ClientRelayerApi<'a> {
    pub async fn create(&self, chain_id: u64, name: &str) -> ApiResult<CreateRelayerResult> {
        self.relayer_api.create(chain_id, name).await
    }

    pub async fn clone_relayer(&self,  id: &RelayerId, chain_id: u64, name: &str) -> ApiResult<CreateRelayerResult> {
        self.relayer_api.clone(id, chain_id, name).await
    }

    pub async fn delete(&self, id: &RelayerId) -> ApiResult<()> {
        self.relayer_api.delete(&id).await
    }

    pub async fn get(&self, id: &RelayerId) -> ApiResult<Option<GetRelayerResult>> {
        self.relayer_api.get(&id).await
    }

    pub async fn get_all(
        &self,
        paging_context: &PagingContext,
        only_for_chain_id: Option<u64>,
    ) -> ApiResult<PagingResult<Relayer>> {
        self.relayer_api.get_all(only_for_chain_id, paging_context).await
    }
}

pub struct ClientNetworkApi<'a> {
    network_api: &'a NetworkApi,
}

impl<'a> ClientNetworkApi<'a> {
    pub async fn get(&self, chain_id: u64) -> ApiResult<Option<Network>> {
        let chain_id = ChainId::new(chain_id);
        self.network_api.get(&chain_id).await
    }

    pub async fn get_all(&self) -> ApiResult<Vec<Network>> {
        self.network_api.get_all().await
    }

    pub async fn get_gas_prices(&self, chain_id: u64) -> ApiResult<Option<GasEstimatorResult>> {
        self.network_api.get_gas_prices(chain_id).await
    }
}

pub struct ClientTransactionApi<'a> {
    transaction_api: &'a TransactionApi,
}

impl<'a> ClientTransactionApi<'a> {
    pub async fn get(&self, transaction_id: &TransactionId) -> ApiResult<Option<Transaction>> {
        self.transaction_api.get(&transaction_id).await
    }

    pub async fn get_status(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<RelayTransactionStatusResult>> {
        self.transaction_api.get_status(&transaction_id).await
    }
}

pub struct ClientAllowlistApi<'a> {
    relayer_api: &'a RelayerApi,
}

impl<'a> ClientAllowlistApi<'a> {
    pub async fn get(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<EvmAddress>> {
        self.relayer_api.allowlist.get_all(&relayer_id, paging_context).await
    }
}

#[derive(Debug, Clone)]
pub struct AdminRelayerClientConfig {
    pub server_url: String,
    pub provider_url: String,
    pub relayer_id: RelayerId,
    pub auth: AdminRelayerClientAuth,
    pub speed: Option<TransactionSpeed>,
}

#[derive(Debug, Clone)]
pub enum AdminRelayerClientAuth {
    BasicAuth { username: String, password: String },
}

#[derive(Debug, Clone)]
pub struct AdminRelayerClient {
    relayer_client: RelayerClient,
    relayer_api: RelayerApi,
}

impl AdminRelayerClient {
    pub fn new(config: AdminRelayerClientConfig) -> Self {
        let relayer_client_config = RelayerClientConfig {
            server_url: config.server_url.clone(),
            relayer_id: config.relayer_id.clone(),
            auth: match &config.auth {
                AdminRelayerClientAuth::BasicAuth { username, password } => {
                    RelayerClientAuth::BasicAuth {
                        username: username.clone(),
                        password: password.clone(),
                    }
                }
            },
            speed: config.speed,
        };

        let relayer_client = RelayerClient::new(relayer_client_config);

        let api_config = ApiBaseConfig {
            server_url: config.server_url,
            auth: match config.auth {
                AdminRelayerClientAuth::BasicAuth { username, password } => {
                    AuthConfig::BasicAuth { username, password }
                }
            },
        };
        let client = Arc::new(HttpClient::new(api_config));
        let relayer_api = RelayerApi::new(client);

        Self { relayer_client, relayer_api }
    }

    pub fn id(&self) -> &RelayerId {
        self.relayer_client.id()
    }

    pub fn speed(&self) -> Option<&TransactionSpeed> {
        self.relayer_client.speed()
    }

    pub async fn address(&self) -> ApiResult<EvmAddress> {
        self.relayer_client.address().await
    }

    pub async fn get_info(&self) -> ApiResult<Relayer> {
        self.relayer_client.get_info().await
    }

    pub fn allowlist(&self) -> RelayerClientAllowlistApi<'_> {
        self.relayer_client.allowlist()
    }

    pub fn sign(&self) -> RelayerClientSignApi<'_> {
        self.relayer_client.sign()
    }

    pub fn transaction(&self) -> AdminRelayerClientTransactionApi<'_> {
        AdminRelayerClientTransactionApi {
            transaction_api: &self.relayer_client.transaction_api,
            relayer_api: &self.relayer_api,
            relayer_id: self.relayer_client.id(),
        }
    }

    pub async fn pause(&self) -> ApiResult<()> {
        self.relayer_api.pause(self.relayer_client.id()).await
    }

    pub async fn unpause(&self) -> ApiResult<()> {
        self.relayer_api.unpause(self.relayer_client.id()).await
    }

    pub async fn update_eip1559_status(&self, status: bool) -> ApiResult<()> {
        self.relayer_api.update_eip1559_status(self.relayer_client.id(), status).await
    }

    pub async fn update_max_gas_price<T: ToString>(&self, cap: T) -> ApiResult<()> {
        self.relayer_api.update_max_gas_price(self.relayer_client.id(), cap).await
    }

    pub async fn remove_max_gas_price(&self) -> ApiResult<()> {
        self.relayer_api.remove_max_gas_price(self.relayer_client.id()).await
    }

    pub async fn clone_relayer(
        &self,
        chain_id: u64,
        name: &str,
    ) -> ApiResult<CreateRelayerResult> {
        self.relayer_api.clone(self.relayer_client.id(), chain_id, name).await
    }
}

#[derive(Debug, Clone)]
pub struct AdminRelayerClientTransactionApi<'a> {
    transaction_api: &'a TransactionApi,
    relayer_api: &'a RelayerApi,
    relayer_id: &'a RelayerId,
}

impl<'a> AdminRelayerClientTransactionApi<'a> {
    pub async fn get(&self, transaction_id: &TransactionId) -> ApiResult<Option<Transaction>> {
        self.transaction_api.get(&transaction_id).await
    }

    pub async fn get_status(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<RelayTransactionStatusResult>> {
        self.transaction_api.get_status(&transaction_id).await
    }

    pub async fn get_all(
        &self,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<Transaction>> {
        self.transaction_api.get_all(self.relayer_id, paging_context).await
    }

    pub async fn replace(
        &self,
        transaction_id: &TransactionId,
        replacement_transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<rrelayer_core::transaction::queue_system::ReplaceTransactionResult> {
        self.transaction_api.replace(&transaction_id, replacement_transaction, rate_limit_key).await
    }

    pub async fn cancel(
        &self,
        transaction_id: &TransactionId,
        rate_limit_key: Option<String>,
    ) -> ApiResult<CancelTransactionResponse> {
        self.transaction_api.cancel(&transaction_id, rate_limit_key).await
    }

    pub async fn send(
        &self,
        transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SendTransactionResult> {
        self.transaction_api.send(self.relayer_id, transaction, rate_limit_key).await
    }

    pub async fn wait_for_transaction_receipt_by_id(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<RelayTransactionStatusResult> {
        loop {
            let result = self.transaction_api.get_status(&transaction_id).await?;
            if let Some(status_result) = result {
                match status_result.status {
                    TransactionStatus::PENDING | TransactionStatus::INMEMPOOL => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    TransactionStatus::MINED
                    | TransactionStatus::CONFIRMED
                    | TransactionStatus::FAILED => {
                        return Ok(status_result);
                    }
                    TransactionStatus::EXPIRED => {
                        return Err(ApiSdkError::ConfigError("Transaction expired".to_string()));
                    }
                    TransactionStatus::CANCELLED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was cancelled".to_string(),
                        ));
                    }
                    TransactionStatus::REPLACED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was replaced".to_string(),
                        ));
                    }
                    TransactionStatus::DROPPED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was dropped from mempool".to_string(),
                        ));
                    }
                }
            } else {
                return Err(ApiSdkError::ConfigError("Transaction not found".to_string()));
            }
        }
    }

    pub async fn get_count(&self, transaction_count_type: TransactionCountType) -> ApiResult<u32> {
        match transaction_count_type {
            TransactionCountType::Pending => {
                self.transaction_api.get_pending_count(self.relayer_id).await
            }
            TransactionCountType::Inmempool => {
                self.transaction_api.get_inmempool_count(self.relayer_id).await
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum TransactionCountType {
    Pending,
    Inmempool,
}

#[derive(Debug, Clone)]
pub struct RelayerClientConfig {
    pub server_url: String,
    pub relayer_id: RelayerId,
    pub auth: RelayerClientAuth,
    pub speed: Option<TransactionSpeed>,
}

#[derive(Debug, Clone)]
pub enum RelayerClientAuth {
    ApiKey { api_key: String },
    BasicAuth { username: String, password: String },
}

#[derive(Debug, Clone)]
pub struct RelayerClient {
    id: RelayerId,
    speed: Option<TransactionSpeed>,
    api_base_config: ApiBaseConfig,
    relayer_api: RelayerApi,
    sign_api: SignApi,
    transaction_api: TransactionApi,
}

impl RelayerClient {
    pub fn new(config: RelayerClientConfig) -> Self {
        let api_base_config = ApiBaseConfig {
            server_url: config.server_url,
            auth: match config.auth {
                RelayerClientAuth::ApiKey { api_key } => AuthConfig::ApiKey { api_key },
                RelayerClientAuth::BasicAuth { username, password } => {
                    AuthConfig::BasicAuth { username, password }
                }
            },
        };
        let client = Arc::new(HttpClient::new(api_base_config.clone()));

        Self {
            id: config.relayer_id,
            speed: config.speed,
            api_base_config,
            relayer_api: RelayerApi::new(Arc::clone(&client)),
            sign_api: SignApi::new(Arc::clone(&client)),
            transaction_api: TransactionApi::new(Arc::clone(&client)),
        }
    }

    pub fn id(&self) -> &RelayerId {
        &self.id
    }

    pub fn speed(&self) -> Option<&TransactionSpeed> {
        self.speed.as_ref()
    }

    pub async fn address(&self) -> ApiResult<EvmAddress> {
        let info = self.get_info().await?;
        Ok(info.address)
    }

    pub async fn get_info(&self) -> ApiResult<Relayer> {
        let result = self.relayer_api.get(&self.id).await?;
        match result {
            Some(get_result) => Ok(get_result.relayer),
            None => Err(ApiSdkError::ConfigError("Relayer not found".to_string())),
        }
    }

    pub fn allowlist(&self) -> RelayerClientAllowlistApi<'_> {
        RelayerClientAllowlistApi { relayer_api: &self.relayer_api, relayer_id: &self.id }
    }

    pub fn sign(&self) -> RelayerClientSignApi<'_> {
        RelayerClientSignApi { sign_api: &self.sign_api, relayer_id: &self.id }
    }

    pub fn transaction(&self) -> RelayerClientTransactionApi<'_> {
        RelayerClientTransactionApi { transaction_api: &self.transaction_api, relayer_id: &self.id }
    }
}

pub struct RelayerClientAllowlistApi<'a> {
    relayer_api: &'a RelayerApi,
    relayer_id: &'a RelayerId,
}

impl<'a> RelayerClientAllowlistApi<'a> {
    pub async fn get(&self, paging_context: &PagingContext) -> ApiResult<PagingResult<EvmAddress>> {
        self.relayer_api.allowlist.get_all(self.relayer_id, paging_context).await
    }
}

pub struct RelayerClientSignApi<'a> {
    sign_api: &'a SignApi,
    relayer_id: &'a RelayerId,
}

impl<'a> RelayerClientSignApi<'a> {
    pub async fn text(
        &self,
        message: &str,
        rate_limit_key: Option<String>,
    ) -> ApiResult<rrelayer_core::signing::SignTextResult> {
        self.sign_api.sign_text(self.relayer_id, message, rate_limit_key).await
    }

    pub async fn text_history(
        &self,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<SignedTextHistory>> {
        self.sign_api.get_signed_text_history(self.relayer_id, paging_context).await
    }

    pub async fn typed_data(
        &self,
        typed_data: &alloy::dyn_abi::TypedData,
        rate_limit_key: Option<String>,
    ) -> ApiResult<rrelayer_core::signing::SignTypedDataResult> {
        self.sign_api.sign_typed_data(self.relayer_id, typed_data, rate_limit_key).await
    }

    pub async fn typed_data_history(
        &self,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<SignedTypedDataHistory>> {
        self.sign_api.get_signed_typed_data_history(self.relayer_id, paging_context).await
    }
}

pub struct RelayerClientTransactionApi<'a> {
    transaction_api: &'a TransactionApi,
    relayer_id: &'a RelayerId,
}

impl<'a> RelayerClientTransactionApi<'a> {
    pub async fn get(&self, transaction_id: &TransactionId) -> ApiResult<Option<Transaction>> {
        self.transaction_api.get(&transaction_id).await
    }

    pub async fn get_status(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<RelayTransactionStatusResult>> {
        self.transaction_api.get_status(&transaction_id).await
    }

    pub async fn get_all(
        &self,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<Transaction>> {
        self.transaction_api.get_all(self.relayer_id, paging_context).await
    }

    pub async fn replace(
        &self,
        transaction_id: &TransactionId,
        replacement_transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<rrelayer_core::transaction::queue_system::ReplaceTransactionResult> {
        self.transaction_api.replace(transaction_id, replacement_transaction, rate_limit_key).await
    }

    pub async fn cancel(
        &self,
        transaction_id: &TransactionId,
        rate_limit_key: Option<String>,
    ) -> ApiResult<CancelTransactionResponse> {
        self.transaction_api.cancel(transaction_id, rate_limit_key).await
    }

    pub async fn send(
        &self,
        transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SendTransactionResult> {
        self.transaction_api.send(self.relayer_id, transaction, rate_limit_key).await
    }

    pub async fn wait_for_transaction_receipt_by_id(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<RelayTransactionStatusResult> {
        loop {
            let result = self.transaction_api.get_status(&transaction_id).await?;
            if let Some(status_result) = result {
                use rrelayer_core::transaction::types::TransactionStatus;
                match status_result.status {
                    TransactionStatus::PENDING | TransactionStatus::INMEMPOOL => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        continue;
                    }
                    TransactionStatus::MINED
                    | TransactionStatus::CONFIRMED
                    | TransactionStatus::FAILED => {
                        return Ok(status_result);
                    }
                    TransactionStatus::EXPIRED => {
                        return Err(ApiSdkError::ConfigError("Transaction expired".to_string()));
                    }
                    TransactionStatus::CANCELLED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was cancelled".to_string(),
                        ));
                    }
                    TransactionStatus::REPLACED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was replaced".to_string(),
                        ));
                    }
                    TransactionStatus::DROPPED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was dropped from mempool".to_string(),
                        ));
                    }
                }
            } else {
                return Err(ApiSdkError::ConfigError("Transaction not found".to_string()));
            }
        }
    }

    pub async fn get_count(&self, transaction_count_type: TransactionCountType) -> ApiResult<u32> {
        match transaction_count_type {
            TransactionCountType::Pending => {
                self.transaction_api.get_pending_count(self.relayer_id).await
            }
            TransactionCountType::Inmempool => {
                self.transaction_api.get_inmempool_count(self.relayer_id).await
            }
        }
    }
}

pub fn create_client(config: CreateClientConfig) -> Client {
    Client::new(config)
}

pub fn create_relayer_client(config: CreateRelayerClientConfig) -> RelayerClient {
    RelayerClient::new(RelayerClientConfig {
        server_url: config.server_url,
        relayer_id: config.relayer_id,
        auth: RelayerClientAuth::ApiKey { api_key: config.api_key },
        speed: config.speed,
    })
}
