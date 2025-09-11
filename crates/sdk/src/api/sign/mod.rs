use rrelayer_core::relayer::{
    api::sign::{SignTextResult, SignTypedDataResult},
    types::RelayerId,
};
use std::sync::Arc;

use crate::api::{http::HttpClient, types::ApiResult};

pub struct SignApi {
    client: Arc<HttpClient>,
}

impl SignApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    pub async fn sign_text(&self, relayer_id: &RelayerId, text: &str) -> ApiResult<SignTextResult> {
        self.client
            .post(
                &format!("relayers/{}/sign/message", relayer_id),
                &serde_json::json!({ "text": text }),
            )
            .await
    }

    pub async fn sign_typed_data(
        &self,
        relayer_id: &RelayerId,
        typed_data: &alloy::dyn_abi::TypedData,
    ) -> ApiResult<SignTypedDataResult> {
        self.client.post(&format!("relayers/{}/sign/typed-data", relayer_id), typed_data).await
    }
}
