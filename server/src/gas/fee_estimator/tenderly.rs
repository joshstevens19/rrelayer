use alloy::primitives::{utils::UnitsError, ParseSignedError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::base::{
    parse_formatted_gas_to_u128, BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult,
    GasPriceResult,
};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::types::ChainId,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TenderlyGasProviderSetupConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TenderlyGasEstimateSpeedResult {
    #[serde(rename = "maxPriorityFeePerGas")]
    max_priority_fee_per_gas: String,

    #[serde(rename = "maxFeePerGas")]
    max_fee_per_gas: String,

    #[serde(rename = "waitTime")]
    wait_time: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TenderlyGasEstimatePriceResult {
    low: TenderlyGasEstimateSpeedResult,

    medium: TenderlyGasEstimateSpeedResult,

    high: TenderlyGasEstimateSpeedResult,
}

impl TenderlyGasEstimatePriceResult {
    fn gas_price_result(
        speed: &TenderlyGasEstimateSpeedResult,
        is_super_fast: bool,
    ) -> Result<GasPriceResult, UnitsError> {
        let (priority_multiplier, wait_multiplier) = if is_super_fast {
            (120, 80) // 120% for fees, 80% for wait times
        } else {
            (100, 100) // No adjustment for other speeds
        };

        let max_priority_fee = parse_formatted_gas_to_u128(&speed.max_priority_fee_per_gas)?
            .checked_mul(priority_multiplier)
            .and_then(|v| v.checked_div(100))
            .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        let max_fee = parse_formatted_gas_to_u128(&speed.max_fee_per_gas)?
            .checked_mul(priority_multiplier)
            .and_then(|v| v.checked_div(100))
            .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        Ok(GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(max_priority_fee),
            max_fee: MaxFee::new(max_fee),
            min_wait_time_estimate: Some(speed.wait_time * wait_multiplier / 100),
            max_wait_time_estimate: Some(speed.wait_time * wait_multiplier / 100),
        })
    }

    pub fn to_base_result(&self) -> Result<GasEstimatorResult, UnitsError> {
        Ok(GasEstimatorResult {
            slow: Self::gas_price_result(&self.low, false)?,
            medium: Self::gas_price_result(&self.medium, false)?,
            fast: Self::gas_price_result(&self.high, false)?,
            super_fast: Self::gas_price_result(&self.high, true)?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TenderlyGasFeeChainConfig {
    rpc_url: String,
    chain_id: ChainId,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TenderlyGasFeeEstimator {
    supported_chains: Vec<TenderlyGasFeeChainConfig>,
    api_key: String,
}

#[derive(Debug, Deserialize)]
struct TenderlyGasEstimateJsonRpcResult {
    result: TenderlyGasEstimateResult,
}

#[derive(Debug, Deserialize)]
struct TenderlyGasEstimateResult {
    // current_block_number: String,
    // base_fee_per_gas: String,
    price: TenderlyGasEstimatePriceResult,
}

impl TenderlyGasFeeEstimator {
    pub fn new(api_key: &str, supported_chains: Vec<TenderlyGasFeeChainConfig>) -> Self {
        Self { supported_chains, api_key: api_key.to_string() }
    }

    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> String {
        let rpc_url = self
            .supported_chains
            .iter()
            .find(|c| c.chain_id == *chain_id)
            .as_ref()
            .expect("Chain not found")
            .rpc_url
            .clone();
        format!("{}/{}", rpc_url, self.api_key)
    }

    async fn request_gas_estimate(
        &self,
        chain_id: &ChainId,
    ) -> Result<TenderlyGasEstimatePriceResult, reqwest::Error> {
        let url = self.build_suggested_gas_price_endpoint(chain_id);
        let client = reqwest::Client::new();

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tenderly_gasPrice",
            "params": []
        });

        let gas_estimate_result: TenderlyGasEstimateJsonRpcResult =
            client.post(url).json(&body).send().await?.json().await?;

        Ok(gas_estimate_result.result.price)
    }
}

#[async_trait]
impl BaseGasFeeEstimator for TenderlyGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let gas_estimate_result = self.request_gas_estimate(chain_id).await?;

        Ok(gas_estimate_result.to_base_result().map_err(GasEstimatorError::UnitsError)?)
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.supported_chains.iter().any(|config| config.chain_id == *chain_id)
    }
}
