use crate::api::{http::HttpClient, types::ApiResult};
use rrelayer_core::authentication::api::StatusResponse;
use std::sync::Arc;

#[derive(Clone)]
pub struct Authentication {
    client: Arc<HttpClient>,
}

impl Authentication {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    /// Test basic authentication by calling the auth status endpoint
    pub async fn test_auth(&self) -> ApiResult<StatusResponse> {
        self.client.get("auth/status").await
    }
}
