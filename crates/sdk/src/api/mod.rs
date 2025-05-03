mod authentication;
mod gas;
pub mod http;
mod network;
mod relayer;
mod sign;
mod transaction;
mod types;
pub use types::{ApiResult, ApiSdkError};
mod user;

pub struct HealthApi {
    client: HttpClient,
}

impl HealthApi {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn check(&self) -> ApiResult<()> {
        self.client.get_status("health").await
    }
}

pub use authentication::Authentication;
pub use gas::GasApi;
pub use network::NetworkApi;
pub use relayer::RelayerApi;
pub use sign::SignApi;
pub use transaction::TransactionApi;
pub use types::ApiBaseConfig;
pub use user::UserApi;

use crate::api::http::HttpClient;
