use crate::api::{http::HttpClient, types::ApiResult};
use rrelayer_core::WalletError::ApiError;
use rrelayer_core::network::{ChainId, Network};
use std::sync::Arc;

#[derive(Clone)]
pub struct NetworkApi {
    client: Arc<HttpClient>,
}

impl NetworkApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    /// Get a single network by ID
    pub async fn get(&self, chain_id: &ChainId) -> ApiResult<Option<Network>> {
        let networks = self.get_all().await?;
        Ok(networks.into_iter().find(|network| network.chain_id == *chain_id))
    }

    /// Get all networks
    pub async fn get_all(&self) -> ApiResult<Vec<Network>> {
        self.client.get("networks").await
    }
}
