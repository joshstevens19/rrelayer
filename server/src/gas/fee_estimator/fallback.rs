use std::sync::Arc;

use alloy::{
    consensus::Transaction,
    eips::{BlockId, BlockNumberOrTag},
    providers::Provider,
    rpc::types::{Block, BlockTransactionsKind, Transaction as RpcTransaction},
    transports::TransportError,
};
use async_trait::async_trait;
use futures::future::join_all;
use strum::IntoEnumIterator;

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
    ) -> Result<Option<Block<RpcTransaction>>, TransportError> {
        let block = self
            .provider
            .get_block(
                BlockId::Number(BlockNumberOrTag::Number(block_num.into())),
                BlockTransactionsKind::Full,
            )
            .await?;

        Ok(block)
    }
}

fn calculate_median(prices: &mut [u128]) -> u128 {
    prices.sort();
    let mid = prices.len() / 2;
    if prices.len() % 2 == 0 {
        (prices[mid - 1] + prices[mid]) / 2
    } else {
        prices[mid]
    }
}

#[async_trait]
impl BaseGasFeeEstimator for FallbackGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        _chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let num_blocks_to_check = 5;
        let latest_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        let block_futures = (0..num_blocks_to_check).map(|i| {
            let block_number = latest_block - i;
            self.get_block_with_txs(block_number.into())
        });

        let blocks = join_all(block_futures)
            .await
            .into_iter()
            .filter_map(Result::ok)
            .flatten()
            .collect::<Vec<_>>();

        let mut priority_fees: Vec<MaxPriorityFee> = Vec::new();
        let mut max_fees: Vec<MaxFee> = Vec::new();
        for block in blocks {
            // TODO! handle error
            let txs = block.transactions.as_transactions().unwrap();
            for tx in txs {
                // If EIP-1559 parameters are available, use them
                if let Some(max_priority_fee_per_gas) = tx.max_priority_fee_per_gas() {
                    priority_fees.push(MaxPriorityFee::new(max_priority_fee_per_gas));
                }

                max_fees.push(MaxFee::new(tx.max_fee_per_gas()));

                if let Some(gas_price) = tx.gas_price() {
                    // Non-EIP-1559 transaction or missing EIP-1559 fields
                    // Use gas_price as a fallback for both priority fee and max fee
                    priority_fees.push(MaxPriorityFee::new(gas_price));
                    // use 1 if max_fee_per_gas is not available
                    max_fees.push(MaxFee::new(1));
                }
            }
        }

        // Calculate the median priority fee and max fee as rough estimates
        let estimated_priority_fee = MaxPriorityFee::new(calculate_median(
            &mut priority_fees.iter().copied().map(u128::from).collect::<Vec<u128>>(),
        ));
        let estimated_max_fee = MaxFee::new(calculate_median(
            &mut max_fees.iter().copied().map(u128::from).collect::<Vec<u128>>(),
        ));

        // Use the estimated fees to populate GasEstimatorResult
        let gas_estimate_result = GasEstimatorResult {
            slow: GasPriceResult {
                max_priority_fee: estimated_priority_fee * 9 / 10,
                max_fee: estimated_max_fee * 9 / 10,
                max_wait_time_estimate: None,
                min_wait_time_estimate: None,
            },
            medium: GasPriceResult {
                max_priority_fee: estimated_priority_fee,
                max_fee: estimated_max_fee,
                max_wait_time_estimate: None,
                min_wait_time_estimate: None,
            },
            fast: GasPriceResult {
                max_priority_fee: estimated_priority_fee * 11 / 10,
                max_fee: estimated_max_fee * 11 / 10,
                max_wait_time_estimate: None,
                min_wait_time_estimate: None,
            },
            super_fast: GasPriceResult {
                max_priority_fee: estimated_priority_fee * 12 / 10,
                max_fee: estimated_max_fee * 12 / 10,
                max_wait_time_estimate: None,
                min_wait_time_estimate: None,
            },
        };

        Ok(gas_estimate_result)
    }

    fn is_chain_supported(&self, _: &ChainId) -> bool {
        true
    }
}
