use rrelayerr_core::gas::fee_estimator::base::GasEstimatorResult;

use crate::api::{http::HttpClient, types::ApiResult};

pub struct GasApi {
    client: HttpClient,
}

impl GasApi {
    pub fn new(client: HttpClient) -> Self {
        Self { client }
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
    pub async fn get_gas_prices(&self, chain_id: u64) -> ApiResult<Option<GasEstimatorResult>> {
        let endpoint = format!("gas/price/{}", chain_id.to_string());
        self.client.get(&endpoint).await
    }
}
