mod allowlist;

use crate::api::{
    http::HttpClient,
    types::{ApiResult, ApiSdkError},
};
pub use allowlist::RelayerAllowlist;
use rrelayer_core::relayer::{CloneRelayerRequest, CreateRelayerRequest};
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    network::ChainId,
    relayer::{CreateRelayerResult, GetRelayerResult, ImportRelayerResult, Relayer, RelayerId},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RelayerApi {
    client: Arc<HttpClient>,
    pub allowlist: RelayerAllowlist,
}

impl RelayerApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { allowlist: RelayerAllowlist::new(client.clone()), client }
    }

    pub async fn get_all(
        &self,
        chain_id: Option<u64>,
        paging: &PagingContext,
    ) -> ApiResult<PagingResult<Relayer>> {
        let mut query = serde_json::Map::new();
        if let Some(chain_id) = chain_id {
            query.insert("chainId".to_string(), chain_id.to_string().into());
        }

        let paging_value = serde_json::to_value(paging)
            .map_err(|e| ApiSdkError::SerializationError(e.to_string()))?;

        query.extend(
            paging_value
                .as_object()
                .ok_or_else(|| {
                    ApiSdkError::SerializationError("Failed to convert paging to object".into())
                })?
                .clone(),
        );

        self.client.get_with_query("relayers", Some(&query)).await
    }

    pub async fn get(&self, id: &RelayerId) -> ApiResult<Option<GetRelayerResult>> {
        self.client.get_or_none(&format!("relayers/{}", id)).await
    }

    pub async fn create(&self, chain_id: u64, name: &str) -> ApiResult<CreateRelayerResult> {
        self.client
            .post(
                &format!("relayers/{}/new", chain_id),
                &CreateRelayerRequest { name: name.to_string() },
            )
            .await
    }

    pub async fn clone(
        &self,
        id: &RelayerId,
        chain_id: u64,
        name: &str,
    ) -> ApiResult<CreateRelayerResult> {
        self.client
            .post(
                &format!("relayers/{}/clone", id),
                &CloneRelayerRequest {
                    new_relayer_name: name.to_string(),
                    chain_id: ChainId::new(chain_id),
                },
            )
            .await
    }

    pub async fn delete(&self, id: &RelayerId) -> ApiResult<()> {
        self.client.delete_status(&format!("relayers/{}", id)).await
    }

    pub async fn pause(&self, id: &RelayerId) -> ApiResult<()> {
        self.client.put_status(&format!("relayers/{}/pause", id), &()).await
    }

    pub async fn unpause(&self, id: &RelayerId) -> ApiResult<()> {
        self.client.put_status(&format!("relayers/{}/unpause", id), &()).await
    }

    pub async fn update_eip1559_status(&self, id: &RelayerId, status: bool) -> ApiResult<()> {
        self.client.put_status(&format!("relayers/{}/gas/eip1559/{}", id, status), &()).await
    }

    pub async fn update_max_gas_price<T: ToString>(&self, id: &RelayerId, cap: T) -> ApiResult<()> {
        self.client.put_status(&format!("relayers/{}/gas/max/{}", id, cap.to_string()), &()).await
    }

    pub async fn remove_max_gas_price(&self, id: &RelayerId) -> ApiResult<()> {
        self.client.put_status(&format!("relayers/{}/gas/max/0", id), &()).await
    }

    pub async fn get_pending_count(&self, id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/pending/count", id)).await
    }

    pub async fn get_inmempool_count(&self, id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/inmempool/count", id)).await
    }

    pub async fn import(
        &self,
        chain_id: u64,
        name: &str,
        key_id: &str,
        address: &EvmAddress,
    ) -> ApiResult<ImportRelayerResult> {
        #[derive(serde::Serialize)]
        struct ImportRelayerRequest {
            name: String,
            #[serde(rename = "keyId")]
            key_id: String,
            address: EvmAddress,
        }

        self.client
            .post(
                &format!("relayers/{}/import", chain_id),
                &ImportRelayerRequest {
                    name: name.to_string(),
                    key_id: key_id.to_string(),
                    address: *address,
                },
            )
            .await
    }
}
