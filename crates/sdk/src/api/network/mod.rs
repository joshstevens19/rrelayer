use crate::api::{http::HttpClient, types::ApiResult};
use rrelayer_core::gas::GasEstimatorResult;
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
        let endpoint = format!("networks/{}", chain_id.to_string());
        self.client.get(&endpoint).await
    }

    /// Get all networks
    pub async fn get_all(&self) -> ApiResult<Vec<Network>> {
        self.client.get("networks").await
    }

    /// Get gas prices for a specific chain ID
    ///
    /// # Arguments
    ///
    /// * `chain_id` - The chain ID to get gas prices for
    ///
    /// # Returns
    ///
    /// Returns a Result containing either the gas prices or an error
    pub async fn get_gas_prices(&self, chain_id: &u64) -> ApiResult<Option<GasEstimatorResult>> {
        let endpoint = format!("networks/gas/price/{}", chain_id.to_string());
        self.client.get(&endpoint).await
    }
}
