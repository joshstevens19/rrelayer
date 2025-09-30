use std::sync::Arc;

use alloy::{eips::BlockNumberOrTag, primitives::utils::parse_units};
use async_trait::async_trait;

use super::base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult, GasPriceResult};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::ChainId,
    provider::RelayerProvider,
};

#[derive(Clone)]
pub struct FallbackGasFeeEstimator {
    provider: Arc<RelayerProvider>,
}

impl FallbackGasFeeEstimator {
    pub fn new(provider: Arc<RelayerProvider>) -> Self {
        FallbackGasFeeEstimator { provider }
    }

    async fn estimate_with_fee_history(
        &self,
        chain_id: &ChainId,
    ) -> Result<(u128, u128), GasEstimatorError> {
        let past_blocks = if chain_id.u64() == 1 || chain_id.u64() == 11155111 { 20 } else { 60 };
        let reward_percentile =
            if chain_id.u64() == 1 || chain_id.u64() == 11155111 { 60.0 } else { 25.0 };

        let fee_history = self
            .provider
            .get_fee_history(past_blocks, BlockNumberOrTag::Latest, &[reward_percentile])
            .await
            .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        let base_fee_per_gas = match fee_history.latest_block_base_fee() {
            Some(base_fee) if base_fee != 0 => base_fee,
            _ => self
                .provider
                .get_block_by_number(BlockNumberOrTag::Latest)
                .await
                .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?
                .ok_or_else(|| {
                    GasEstimatorError::CustomError("Latest block not found".to_string())
                })?
                .header
                .base_fee_per_gas
                .ok_or_else(|| {
                    GasEstimatorError::CustomError("EIP-1559 not supported".to_string())
                })?
                .into(),
        };

        let priority_fee = if let Some(rewards) = &fee_history.reward {
            if !rewards.is_empty() {
                let mut all_rewards: Vec<u128> = rewards
                    .iter()
                    .filter_map(|block_rewards| block_rewards.first().copied())
                    .collect();

                if !all_rewards.is_empty() {
                    all_rewards.sort();
                    let median_idx = all_rewards.len() / 2;
                    all_rewards[median_idx]
                } else {
                    if chain_id.u64() == 1 {
                        parse_units("2", "gwei").unwrap().try_into().unwrap() // 2 gwei default for Ethereum
                    } else {
                        parse_units("0.01", "gwei").unwrap().try_into().unwrap()
                        // 0.01 gwei default for other chains
                    }
                }
            } else {
                if chain_id.u64() == 1 {
                    parse_units("2", "gwei").unwrap().try_into().unwrap() // 2 gwei default for Ethereum
                } else {
                    parse_units("0.01", "gwei").unwrap().try_into().unwrap() // 0.01 gwei default for other chains
                }
            }
        } else {
            if chain_id.u64() == 1 {
                parse_units("2", "gwei").unwrap().try_into().unwrap() // 2 gwei default for Ethereum
            } else {
                parse_units("0.01", "gwei").unwrap().try_into().unwrap() // 0.01 gwei default for other chains
            }
        };

        let max_fee = if chain_id.u64() == 1 {
            (base_fee_per_gas + priority_fee).max(priority_fee * 2) // Original logic for Ethereum
        } else {
            base_fee_per_gas + (priority_fee * 2) // Simplified for other chains
        };

        Ok((priority_fee, max_fee))
    }
}

#[async_trait]
impl BaseGasFeeEstimator for FallbackGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        _chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let (base_priority_fee, base_max_fee) =
            match self.estimate_with_fee_history(_chain_id).await {
                Ok(fees) => fees,
                Err(_) => {
                    let suggested = self
                        .provider
                        .estimate_eip1559_fees()
                        .await
                        .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

                    let priority_fee = suggested.max_priority_fee_per_gas;
                    let max_fee = if _chain_id.u64() == 1 {
                        suggested.max_fee_per_gas.max(priority_fee * 2) // Original logic for Ethereum
                    } else {
                        suggested.max_fee_per_gas // Simplified for other chains
                    };
                    (priority_fee, max_fee)
                }
            };

        Ok(GasEstimatorResult {
            slow: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 80) / 100), // -20%
                max_fee: MaxFee::new((base_max_fee * 90) / 100),                       // -10%
                min_wait_time_estimate: Some(120),                                     // 2 minutes
                max_wait_time_estimate: Some(300),                                     // 5 minutes
            },
            medium: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new(base_priority_fee),
                max_fee: MaxFee::new(base_max_fee),
                min_wait_time_estimate: Some(30),  // 30 seconds
                max_wait_time_estimate: Some(120), // 2 minutes
            },
            fast: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 130) / 100), // +30%
                max_fee: MaxFee::new((base_max_fee * 120) / 100),                       // +20%
                min_wait_time_estimate: Some(15), // 15 seconds
                max_wait_time_estimate: Some(60), // 1 minute
            },
            super_fast: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 180) / 100), // +80%
                max_fee: MaxFee::new((base_max_fee * 150) / 100),                       // +50%
                min_wait_time_estimate: Some(5),                                        // 5 seconds
                max_wait_time_estimate: Some(30), // 30 seconds
            },
        })
    }

    fn is_chain_supported(&self, _: &ChainId) -> bool {
        true
    }
}
