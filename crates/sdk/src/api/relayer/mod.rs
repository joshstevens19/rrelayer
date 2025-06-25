mod allowlist;
mod api_keys;

pub use allowlist::RelayerAllowlist;
pub use api_keys::RelayerApiKeys;
use rrelayerr_core::{
    common_types::{PagingContext, PagingResult},
    relayer::{
        api::{CreateRelayerResult, GetRelayerResult},
        types::{Relayer, RelayerId},
    },
};

use crate::api::{
    http::HttpClient,
    types::{ApiResult, ApiSdkError},
};

pub struct RelayerApi {
    client: HttpClient,
    pub api_keys: RelayerApiKeys,
    pub allowlist: RelayerAllowlist,
}

impl RelayerApi {
    pub fn new(client: HttpClient) -> Self {
        Self {
            api_keys: RelayerApiKeys::new(client.clone()),
            allowlist: RelayerAllowlist::new(client.clone()),
            client,
        }
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

        // Handle the serde_json conversion error explicitly
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
        self.client.get(&format!("relayers/{}", id)).await
    }

    pub async fn create(&self, chain_id: u64, name: &str) -> ApiResult<CreateRelayerResult> {
        self.client
            .post(
                &format!("relayers/{}/new", chain_id.to_string()),
                &serde_json::json!({ "name": name }),
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
                &serde_json::json!({ "new_relayer_name": name, "chain_id": chain_id }),
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
}
