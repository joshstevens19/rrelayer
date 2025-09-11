use serde::{Deserialize, Serialize};

use crate::api::{http::HttpClient, types::ApiResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub authenticated: bool,
    pub message: String,
}

pub struct Authentication {
    client: HttpClient,
}

impl Authentication {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    /// Test basic authentication by calling the auth status endpoint
    pub async fn test_auth(&self) -> ApiResult<StatusResponse> {
        self.client.get("auth/status").await
    }
}