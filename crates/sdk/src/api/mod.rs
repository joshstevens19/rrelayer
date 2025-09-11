mod authentication;
mod gas;
pub mod http;
mod network;
mod relayer;
mod sign;
mod transaction;
pub mod types;

use std::sync::Arc;
pub use types::{ApiResult, ApiSdkError};

pub struct HealthApi {
    client: Arc<HttpClient>,
}

impl HealthApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
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

use crate::api::http::HttpClient;
