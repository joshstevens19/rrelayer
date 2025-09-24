use rrelayer_core::network::Network;
use std::sync::Arc;

use crate::api::{http::HttpClient, types::ApiResult};

#[derive(Clone)]
pub struct NetworkApi {
    client: Arc<HttpClient>,
}

impl NetworkApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    /// Get all networks
    pub async fn get_all_networks(&self) -> ApiResult<Vec<Network>> {
        self.client.get("networks").await
    }
}
