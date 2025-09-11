use rrelayer_core::network::types::Network;
use std::sync::Arc;

use crate::api::{http::HttpClient, types::ApiResult};

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

    /// Get enabled networks
    pub async fn get_enabled_networks(&self) -> ApiResult<Vec<Network>> {
        self.client.get("networks/enabled").await
    }

    /// Get disabled networks
    pub async fn get_disabled_networks(&self) -> ApiResult<Vec<Network>> {
        self.client.get("networks/disabled").await
    }

    /// Enable a network
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID of the network to enable
    pub async fn enable_network(&self, chain_id: u64) -> ApiResult<()> {
        self.client.put_status(&format!("networks/enable/{}", chain_id.to_string()), &()).await
    }

    /// Disable a network
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID of the network to disable
    pub async fn disable_network(&self, chain_id: u64) -> ApiResult<()> {
        self.client.put_status(&format!("networks/disable/{}", chain_id.to_string()), &()).await
    }
}
