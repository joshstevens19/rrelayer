use alloy::primitives::utils::UnitsError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::base::{
    parse_formatted_gas_to_u128, BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult,
    GasPriceResult,
};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::types::{Chain, ChainId},
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
    pub supported_chains: Vec<Chain>,
    pub auth_header: Option<String>,
}

impl CustomGasFeeEstimator {
    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> String {
        format!("{}/{}", self.endpoint, chain_id)
    }

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

    fn get_supported_chains(&self) -> Vec<Chain> {
        self.supported_chains.clone()
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.supported_chains.iter().any(|&id| chain_id == &id.chain_id())
    }
}
