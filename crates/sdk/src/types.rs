use rrelayerr_core::authentication::types::TokenPair;

use crate::api::ApiBaseConfig;

#[derive(Clone)]
pub struct SdkContext {
    pub config: ApiBaseConfig,
    pub token_pair: Option<TokenPair>,
}

impl SdkContext {
    pub fn new(server_url: String) -> Self {
        Self { config: ApiBaseConfig::Basic { server_url }, token_pair: None }
    }

    pub(crate) fn with_auth_token(&self, token: String) -> ApiBaseConfig {
        ApiBaseConfig::WithAuthToken {
            server_url: match &self.config {
                ApiBaseConfig::Basic { server_url }
                | ApiBaseConfig::WithAuthToken { server_url, .. }
                | ApiBaseConfig::WithApiKey { server_url, .. } => server_url.clone(),
            },
            auth_token: token,
        }
    }
}
