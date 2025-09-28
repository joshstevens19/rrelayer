use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult, GasPriceResult};

const GWEI_TO_WEI: u128 = 1_000_000_000;
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::ChainId,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockNativeGasProviderSetupConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct BlockNativeGasEstimateResult {
    #[serde(rename = "estimatedPrices")]
    estimated_prices: Vec<BlockNativeEstimatedPrice>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct BlockNativeEstimatedPrice {
    confidence: u8,
    price: u64,
    #[serde(rename = "maxPriorityFeePerGas")]
    max_priority_fee_per_gas: u64,
    #[serde(rename = "maxFeePerGas")]
    max_fee_per_gas: u64,
}

impl BlockNativeGasEstimateResult {
    fn get_estimate_by_confidence(&self, confidence: u8) -> Option<&BlockNativeEstimatedPrice> {
        self.estimated_prices.iter().find(|price| price.confidence == confidence)
    }

    pub fn to_base_result(&self) -> Result<GasEstimatorResult, GasEstimatorError> {
        // BlockNative typically provides confidence levels: 70, 80, 90, 95, 99
        let slow = self
            .get_estimate_by_confidence(70)
            .or_else(|| self.estimated_prices.first())
            .ok_or_else(|| {
                GasEstimatorError::CustomError("No gas estimates available".to_string())
            })?;

        let medium = self
            .get_estimate_by_confidence(80)
            .or_else(|| self.get_estimate_by_confidence(70))
            .or_else(|| self.estimated_prices.first())
            .ok_or_else(|| {
                GasEstimatorError::CustomError("No gas estimates available".to_string())
            })?;

        let fast = self
            .get_estimate_by_confidence(90)
            .or_else(|| self.get_estimate_by_confidence(95))
            .or_else(|| self.estimated_prices.last())
            .ok_or_else(|| {
                GasEstimatorError::CustomError("No gas estimates available".to_string())
            })?;

        let super_fast = self
            .get_estimate_by_confidence(95)
            .or_else(|| self.get_estimate_by_confidence(99))
            .or_else(|| self.estimated_prices.last())
            .ok_or_else(|| {
                GasEstimatorError::CustomError("No gas estimates available".to_string())
            })?;

        let slow_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(
                slow.max_priority_fee_per_gas as u128 * GWEI_TO_WEI,
            ),
            max_fee: MaxFee::new(slow.max_fee_per_gas as u128 * GWEI_TO_WEI),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        };

        let medium_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(
                medium.max_priority_fee_per_gas as u128 * GWEI_TO_WEI,
            ),
            max_fee: MaxFee::new(medium.max_fee_per_gas as u128 * GWEI_TO_WEI),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        };

        let fast_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(
                fast.max_priority_fee_per_gas as u128 * GWEI_TO_WEI,
            ),
            max_fee: MaxFee::new(fast.max_fee_per_gas as u128 * GWEI_TO_WEI),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        };

        // For super fast, add 20% buffer to the highest confidence estimate
        let super_fast_priority =
            (super_fast.max_priority_fee_per_gas as u128 * 120 / 100) * GWEI_TO_WEI;
        let super_fast_max = (super_fast.max_fee_per_gas as u128 * 120 / 100) * GWEI_TO_WEI;

        let super_fast_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(super_fast_priority),
            max_fee: MaxFee::new(super_fast_max),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        };

        Ok(GasEstimatorResult {
            slow: slow_result,
            medium: medium_result,
            fast: fast_result,
            super_fast: super_fast_result,
        })
    }
}

pub struct BlockNativeGasFeeEstimator {
    config: BlockNativeGasProviderSetupConfig,
    client: reqwest::Client,
}

impl BlockNativeGasFeeEstimator {
    pub fn new(config: BlockNativeGasProviderSetupConfig) -> Result<Self, GasEstimatorError> {
        let client = reqwest::Client::new();
        Ok(Self { config, client })
    }
}

#[async_trait]
impl BaseGasFeeEstimator for BlockNativeGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let url =
            format!("https://api.blocknative.com/gasprices/blockprices?chainid={}", chain_id.u64());

        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.config.api_key)
            .send()
            .await
            .map_err(|e| GasEstimatorError::ReqwestError(e))?;

        if !response.status().is_success() {
            return Err(GasEstimatorError::CustomError(format!(
                "BlockNative API returned status: {}",
                response.status()
            )));
        }

        let gas_estimates: BlockNativeGasEstimateResult =
            response.json().await.map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        gas_estimates.to_base_result()
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        // BlockNative supports major EVM chains
        // Common supported chains: Ethereum (1), Polygon (137), BSC (56), Optimism (10), Arbitrum (42161)
        matches!(chain_id.u64(), 1 | 10 | 56 | 137 | 42161 | 8453 | 43114)
    }
}
