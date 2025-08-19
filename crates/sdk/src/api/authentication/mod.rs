use alloy::primitives::Address;
use rrelayer_core::authentication::{
    api::{AuthenticateRequest, GenerateSecretResult},
    types::TokenPair,
};
use serde::Serialize;

use crate::api::{http::HttpClient, types::ApiResult};

pub struct Authentication {
    client: HttpClient,
}

impl Authentication {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn generate_auth_secret(&self, address: &Address) -> ApiResult<GenerateSecretResult> {
        #[derive(Serialize)]
        struct Request {
            address: String,
        }

        self.client
            .post("authentication/secret/generate", &Request { address: address.to_string() })
            .await
    }

    pub async fn authenticate(&self, request: AuthenticateRequest) -> ApiResult<TokenPair> {
        self.client.post("authentication/authenticate", &request).await
    }

    pub async fn refresh_auth_token(&self, token: &str) -> ApiResult<TokenPair> {
        #[derive(Serialize)]
        struct Request {
            token: String,
        }

        self.client.post("authentication/refresh", &Request { token: token.to_string() }).await
    }
}
