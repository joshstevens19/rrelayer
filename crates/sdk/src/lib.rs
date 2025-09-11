mod api;

pub use api::{
    ApiSdkError, Authentication, GasApi, NetworkApi, RelayerApi, SignApi, TransactionApi, UserApi,
};

use crate::api::{ApiResult, HealthApi, http::HttpClient, types::ApiBaseConfig};

pub struct SDK {
    pub auth: Authentication,
    pub gas: GasApi,
    pub network: NetworkApi,
    pub relayer: RelayerApi,
    pub sign: SignApi,
    pub transaction: TransactionApi,
    pub user: UserApi,
    pub health: HealthApi,
}

impl SDK {
    /// Create a new SDK instance with basic authentication
    pub fn new(server_url: String, username: String, password: String) -> Self {
        let config = ApiBaseConfig {
            server_url,
            username,
            password,
        };
        let client = HttpClient::new(config);

        Self {
            auth: Authentication::new(client.clone()),
            gas: GasApi::new(client.clone()),
            network: NetworkApi::new(client.clone()),
            relayer: RelayerApi::new(client.clone()),
            sign: SignApi::new(client.clone()),
            transaction: TransactionApi::new(client.clone()),
            user: UserApi::new(client.clone()),
            health: HealthApi::new(client.clone()),
        }
    }

    /// Test that basic authentication is working
    pub async fn test_auth(&self) -> ApiResult<()> {
        self.auth.test_auth().await?;
        Ok(())
    }
}