use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::{http::HttpClient, types::ApiResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub authenticated: bool,
    pub message: String,
}

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
