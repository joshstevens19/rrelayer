use alloy::primitives::utils::UnitsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::base::{
    parse_formatted_gas_to_u128, BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult,
    GasPriceResult,
};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::ChainId,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CustomGasEstimateSpeedResult {
    #[serde(rename = "suggestedMaxPriorityFeePerGas")]
    suggested_max_priority_fee_per_gas: String,

    #[serde(rename = "suggestedMaxFeePerGas")]
    suggested_max_fee_per_gas: String,

    #[serde(rename = "minWaitTimeEstimate")]
    min_wait_time_estimate: Option<i64>,

    #[serde(rename = "maxWaitTimeEstimate")]
    max_wait_time_estimate: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CustomGasEstimateResult {
    slow: CustomGasEstimateSpeedResult,
    medium: CustomGasEstimateSpeedResult,
    fast: CustomGasEstimateSpeedResult,
    #[serde(rename = "superFast")]
    super_fast: CustomGasEstimateSpeedResult,
}

impl CustomGasEstimateResult {
    /// Converts a custom gas estimate speed result to the standard gas price result format.
    ///
    /// # Arguments
    /// * `speed` - The custom gas estimate data for a specific speed
    ///
    /// # Returns
    /// * `Ok(GasPriceResult)` - The converted standard gas price result
    /// * `Err(UnitsError)` - If parsing the gas price strings fails
    fn gas_price_result(
        speed: &CustomGasEstimateSpeedResult,
    ) -> Result<GasPriceResult, UnitsError> {
        Ok(GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(parse_formatted_gas_to_u128(
                &speed.suggested_max_priority_fee_per_gas,
            )?),
            max_fee: MaxFee::new(parse_formatted_gas_to_u128(&speed.suggested_max_fee_per_gas)?),
            min_wait_time_estimate: speed.min_wait_time_estimate,
            max_wait_time_estimate: speed.max_wait_time_estimate,
        })
    }

    /// Converts the custom gas estimate result to the standard gas estimator result format.
    ///
    /// # Returns
    /// * `Ok(GasEstimatorResult)` - The converted standard gas estimator result
    /// * `Err(UnitsError)` - If parsing any of the gas price strings fails
    pub fn to_base_result(&self) -> Result<GasEstimatorResult, UnitsError> {
        Ok(GasEstimatorResult {
            slow: Self::gas_price_result(&self.slow)?,
            medium: Self::gas_price_result(&self.medium)?,
            fast: Self::gas_price_result(&self.fast)?,
            super_fast: Self::gas_price_result(&self.super_fast)?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CustomGasFeeEstimator {
    pub endpoint: String,
    pub supported_chains: Vec<ChainId>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub auth_header: Option<String>,
}

impl CustomGasFeeEstimator {
    /// Builds the API endpoint URL for requesting gas prices for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to build the endpoint for
    ///
    /// # Returns
    /// * `String` - The complete endpoint URL
    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> String {
        format!("{}/{}", self.endpoint, chain_id)
    }

    /// Requests gas price estimates from the custom API endpoint.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to request gas prices for
    ///
    /// # Returns
    /// * `Ok(CustomGasEstimateResult)` - The gas estimates from the custom API
    /// * `Err(reqwest::Error)` - If the HTTP request fails
    async fn request_gas_estimate(
        &self,
        chain_id: &ChainId,
    ) -> Result<CustomGasEstimateResult, reqwest::Error> {
        let url = self.build_suggested_gas_price_endpoint(chain_id);
        let client = reqwest::Client::new();

        let mut gas_estimate_result = client.get(url).header("Accept", "application/json");

        if let Some(auth_header) = &self.auth_header {
            gas_estimate_result = gas_estimate_result.header("Authorization", auth_header);
        }

        let gas_estimate_result: CustomGasEstimateResult = gas_estimate_result
            .send()
            .await? // Await the response
            .json()
            .await?;

        Ok(gas_estimate_result)
    }
}

#[async_trait]
impl BaseGasFeeEstimator for CustomGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let gas_estimate_result = self.request_gas_estimate(chain_id).await?;

        Ok(gas_estimate_result.to_base_result().map_err(GasEstimatorError::UnitsError)?)
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.supported_chains.iter().any(|&id| chain_id == &id)
    }
}
