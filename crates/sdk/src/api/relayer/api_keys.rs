use crate::api::{http::HttpClient, types::ApiResult};
use rrelayerr_core::common_types::ApiKey;
use rrelayerr_core::relayer::api::CreateRelayerApiResult;
use rrelayerr_core::{
    common_types::{PagingContext, PagingResult},
    relayer::types::RelayerId,
};

pub struct RelayerApiKeys {
    client: HttpClient,
}

impl RelayerApiKeys {
    pub(crate) fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn create(&self, relayer_id: &RelayerId) -> ApiResult<CreateRelayerApiResult> {
        self.client.post(&format!("relayers/{}/api-keys", relayer_id), &()).await
    }

    pub async fn delete(&self, relayer_id: &RelayerId, api_key: &ApiKey) -> ApiResult<()> {
        self.client
            .post_status(
                &format!("relayers/{}/api-keys/delete", relayer_id),
                &serde_json::json!({ "apiKey": api_key }),
            )
            .await
    }

    pub async fn get_all(
        &self,
        relayer_id: &RelayerId,
        paging: &PagingContext,
    ) -> ApiResult<PagingResult<ApiKey>> {
        self.client.get_with_query(&format!("relayers/{}/api-keys", relayer_id), Some(paging)).await
    }
}
