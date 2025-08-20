use std::sync::Arc;

use alloy::network::AnyRpcBlock;
use alloy::transports::TransportResult;
use alloy::{
    consensus::Transaction,
    eips::{BlockId, BlockNumberOrTag},
    providers::Provider,
    rpc::types::{Block, BlockTransactionsKind, Transaction as RpcTransaction},
    transports::TransportError,
};
use async_trait::async_trait;
use futures::future::join_all;

use super::base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult, GasPriceResult};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::types::ChainId,
    provider::RelayerProvider,
    shared::common_types::BlockNumber,
};

#[derive(Clone)]
pub struct FallbackGasFeeEstimator {
    provider: Arc<RelayerProvider>,
}

impl FallbackGasFeeEstimator {
    pub fn new(provider: Arc<RelayerProvider>) -> Self {
        FallbackGasFeeEstimator { provider }
    }

    pub async fn get_block_with_txs(
        &self,
        block_num: BlockNumber,
    ) -> Result<Option<AnyRpcBlock>, TransportError> {
        let block = self
            .provider
            .get_block(
                BlockId::Number(BlockNumberOrTag::Number(block_num.into())),
                BlockTransactionsKind::Full,
            )
            .await?;

        Ok(block)
    }

    fn get_safe_block_range(&self, latest_block: u64, num_blocks: u64) -> Vec<u64> {
        let start_block = latest_block.saturating_sub(num_blocks - 1);
        (start_block..=latest_block).collect()
    }
}

fn calculate_median(prices: &mut [u128]) -> u128 {
    if prices.is_empty() {
        return 0;
    }

    prices.sort();
    let mid = prices.len() / 2;
    if prices.len() % 2 == 0 {
        (prices[mid - 1] + prices[mid]) / 2
    } else {
        prices[mid]
    }
}

fn calculate_percentile(prices: &mut [u128], percentile: f64) -> u128 {
    if prices.is_empty() {
        return 0;
    }

    prices.sort();
    let index = ((prices.len() as f64 - 1.0) * percentile).round() as usize;
    prices[index.min(prices.len() - 1)]
}

#[async_trait]
impl BaseGasFeeEstimator for FallbackGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        _chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let num_blocks_to_check = 5u64;
        let latest_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        // Get safe block range to avoid underflow
        let block_numbers = self.get_safe_block_range(latest_block, num_blocks_to_check);

        if block_numbers.is_empty() {
            return Err(GasEstimatorError::CustomError(
                "No blocks available for gas estimation".to_string(),
            ));
        }

        let block_futures =
            block_numbers.iter().map(|&block_number| self.get_block_with_txs(block_number.into()));

        let blocks = join_all(block_futures)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .flatten()
            .collect::<Vec<_>>();

        if blocks.is_empty() {
            return Err(GasEstimatorError::CustomError(
                "No blocks retrieved for gas estimation".to_string(),
            ));
        }

        let mut priority_fees: Vec<u128> = Vec::new();
        let mut max_fees: Vec<u128> = Vec::new();
        let mut legacy_gas_prices: Vec<u128> = Vec::new();

        for block in blocks {
            let txs = block.transactions.as_transactions().ok_or_else(|| {
                GasEstimatorError::CustomError("Failed to get transactions".to_string())
            })?;

            for tx in txs {
                match (tx.max_priority_fee_per_gas(), tx.max_fee_per_gas(), tx.gas_price()) {
                    // EIP-1559 transaction with all fields
                    (Some(priority_fee), max_fee, _) => {
                        priority_fees.push(priority_fee);
                        max_fees.push(max_fee);
                    }
                    // Legacy transaction
                    (None, _, Some(gas_price)) => {
                        legacy_gas_prices.push(gas_price);
                    }
                    // Fallback case
                    _ => {
                        // Skip transactions without proper gas pricing
                        continue;
                    }
                }
            }
        }

        // If we have no transactions at all, return error
        if priority_fees.is_empty() && legacy_gas_prices.is_empty() {
            return Err(GasEstimatorError::CustomError(
                "No transactions found for gas estimation".to_string(),
            ));
        }

        // Calculate base fees using different approaches based on available data
        let (base_priority_fee, base_max_fee) = if !priority_fees.is_empty() {
            // Use EIP-1559 data
            let median_priority = calculate_median(&mut priority_fees);
            let median_max = calculate_median(&mut max_fees);
            (median_priority, median_max)
        } else {
            // Fallback to legacy gas prices
            let median_legacy = calculate_median(&mut legacy_gas_prices);
            // For legacy transactions, estimate priority fee as a portion of total gas price
            let estimated_priority = median_legacy / 10; // Assume 10% priority fee
            (estimated_priority, median_legacy)
        };

        // Ensure minimum values
        let base_priority_fee = base_priority_fee.max(1_000_000_000); // 1 gwei minimum
        let base_max_fee = base_max_fee.max(base_priority_fee * 2); // At least 2x priority fee

        // Create gas estimates with better scaling factors
        let gas_estimate_result = GasEstimatorResult {
            slow: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 80) / 100), // -20%
                max_fee: MaxFee::new((base_max_fee * 90) / 100),                       // -10%
                max_wait_time_estimate: Some(300),                                     // 5 minutes
                min_wait_time_estimate: Some(120),                                     // 2 minutes
            },
            medium: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new(base_priority_fee),
                max_fee: MaxFee::new(base_max_fee),
                max_wait_time_estimate: Some(120), // 2 minutes
                min_wait_time_estimate: Some(30),  // 30 seconds
            },
            fast: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 130) / 100), // +30%
                max_fee: MaxFee::new((base_max_fee * 120) / 100),                       // +20%
                max_wait_time_estimate: Some(60),                                       // 1 minute
                min_wait_time_estimate: Some(15), // 15 seconds
            },
            super_fast: GasPriceResult {
                max_priority_fee: MaxPriorityFee::new((base_priority_fee * 180) / 100), // +80%
                max_fee: MaxFee::new((base_max_fee * 150) / 100),                       // +50%
                max_wait_time_estimate: Some(30), // 30 seconds
                min_wait_time_estimate: Some(5),  // 5 seconds
            },
        };

        Ok(gas_estimate_result)
    }

    fn is_chain_supported(&self, _: &ChainId) -> bool {
        true
    }
}
