mod api;

pub use api::{
    ApiSdkError, Authentication, GasApi, NetworkApi, RelayerApi, SignApi, TransactionApi,
};
use std::sync::Arc;

use crate::api::{ApiResult, HealthApi, http::HttpClient, types::ApiBaseConfig};

pub struct SDK {
    pub auth: Authentication,
    pub gas: GasApi,
    pub network: NetworkApi,
    pub relayer: RelayerApi,
    pub sign: SignApi,
    pub transaction: TransactionApi,
    pub health: HealthApi,
}

impl SDK {
    /// Create a new SDK instance with basic authentication
    pub fn new(server_url: String, username: String, password: String) -> Self {
        let config = ApiBaseConfig { server_url, username, password };
        let client = Arc::new(HttpClient::new(config));

        Self {
            auth: Authentication::new(Arc::clone(&client)),
            gas: GasApi::new(Arc::clone(&client)),
            network: NetworkApi::new(Arc::clone(&client)),
            relayer: RelayerApi::new(Arc::clone(&client)),
            sign: SignApi::new(Arc::clone(&client)),
            transaction: TransactionApi::new(Arc::clone(&client)),
            health: HealthApi::new(Arc::clone(&client)),
        }
    }

    /// Test that basic authentication is working
    pub async fn test_auth(&self) -> ApiResult<()> {
        self.auth.test_auth().await?;
        Ok(())
    }
}
