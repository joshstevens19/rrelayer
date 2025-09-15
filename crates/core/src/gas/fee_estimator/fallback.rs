use std::sync::Arc;

use alloy::network::{AnyRpcBlock, TransactionResponse};
use alloy::{
    consensus::Transaction,
    eips::{BlockId, BlockNumberOrTag},
    providers::Provider,
    rpc::types::BlockTransactionsKind,
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
    /// Creates a new fallback gas fee estimator using the provided RPC provider.
    ///
    /// # Arguments
    /// * `provider` - The RPC provider to use for blockchain queries
    ///
    /// # Returns
    /// * A new `FallbackGasFeeEstimator` instance
    pub fn new(provider: Arc<RelayerProvider>) -> Self {
        FallbackGasFeeEstimator { provider }
    }

    /// Retrieves a block with full transaction details for the specified block number.
    ///
    /// # Arguments
    /// * `block_num` - The block number to retrieve
    ///
    /// # Returns
    /// * `Ok(Some(AnyRpcBlock))` - The block with transactions if found
    /// * `Ok(None)` - If the block doesn't exist
    /// * `Err(TransportError)` - If the RPC call fails
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

    /// Generates a safe block range to prevent underflow when calculating historical blocks.
    ///
    /// # Arguments
    /// * `latest_block` - The latest block number
    /// * `num_blocks` - The number of blocks to include in the range
    ///
    /// # Returns
    /// * `Vec<u64>` - A vector of block numbers in the safe range
    fn get_safe_block_range(&self, latest_block: u64, num_blocks: u64) -> Vec<u64> {
        let start_block = latest_block.saturating_sub(num_blocks - 1);
        (start_block..=latest_block).collect()
    }
}

/// Calculates the median value from a slice of prices.
///
/// # Arguments
/// * `prices` - A mutable slice of price values (will be sorted in-place)
///
/// # Returns
/// * `u128` - The median value, or 0 if the slice is empty
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

/// Calculates a specific percentile from a slice of prices.
///
/// # Arguments
/// * `prices` - A mutable slice of price values (will be sorted in-place)
/// * `percentile` - The percentile to calculate (0.0 to 1.0)
///
/// # Returns
/// * `u128` - The value at the specified percentile, or 0 if the slice is empty
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
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        // println!("=== STARTING GAS ESTIMATION for chain {} ===", chain_id.u64());
        let latest_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        // println!("=== GAS ESTIMATION DEBUG ===");
        // println!("Latest block: {}", latest_block);
        
        // Smart block selection: look for blocks with transactions
        let mut block_numbers = Vec::new();
        let max_lookback = 50u64; // Don't go back more than 50 blocks
        
        for i in 0..max_lookback {
            let block_num = latest_block.saturating_sub(i);
            if block_num == 0 {
                break;
            }
            
            // Quick check if this block has transactions by looking at gas used
            if let Ok(Some(block)) = self.provider.get_block(
                BlockId::Number(BlockNumberOrTag::Number(block_num.into())), 
                BlockTransactionsKind::Full  // We need Full to get transaction details
            ).await {
                if block.header.gas_used > 0 {
                    block_numbers.push(block_num);
                    // println!("Found block with transactions: {} (gas_used: {})", block_num, block.header.gas_used);
                    if block_numbers.len() >= 10 {  // Collect up to 10 blocks with transactions
                        break;
                    }
                }
            }
        }
        
        // println!("Selected blocks with transactions: {:?}", block_numbers);
        // println!("============================");

        if block_numbers.is_empty() {
            return Err(GasEstimatorError::CustomError(
                "No blocks with transactions found for gas estimation".to_string(),
            ));
        }

        let block_futures =
            block_numbers.iter().map(|&block_number| self.get_block_with_txs(block_number.into()));

        let block_results = join_all(block_futures).await;
        
        // println!("Block fetch results: {} requests made", block_results.len());
        // for (i, result) in block_results.iter().enumerate() {
        //     match result {
        //         Ok(Some(block)) => println!("Block {}: OK, number={:?}", i, block.header.number),
        //         Ok(None) => println!("Block {}: No block found", i),
        //         Err(e) => println!("Block {}: Error - {}", i, e),
        //     }
        // }
        
        let blocks = block_results
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
            // println!("=== BLOCK DEBUG ===");
            // println!("Block number: {:?}", block.header.number);
            // println!("Block hash: {:?}", block.header.hash);
            // println!("Block transactions count (header): {:?}", block.header.transactions_root);
            // println!("Block gas used: {:?}", block.header.gas_used);
            // println!("Block transactions type: {:?}", std::mem::discriminant(&block.transactions));
            
            // Let's also manually check what anvil returns for this block
            let block_num = block.header.number;
            // println!("*** MANUAL RPC CHECK for block {} ***", block_num);
            
            // Make a direct RPC call to double-check
            if let Ok(direct_block) = self.provider.get_block(
                BlockId::Number(BlockNumberOrTag::Number(block_num.into())), 
                BlockTransactionsKind::Full
            ).await {
                if let Some(direct_block) = direct_block {
                    // println!("DIRECT RPC: Block {} gas_used={:?}", block_num, direct_block.header.gas_used);
                    if let Some(direct_txs) = direct_block.transactions.as_transactions() {
                        // println!("DIRECT RPC: {} transactions found", direct_txs.len());
                    } else {
                        // println!("DIRECT RPC: transaction hashes only");
                    }
                } else {
                    // println!("DIRECT RPC: Block {} not found", block_num);
                }
            } else {
                // println!("DIRECT RPC: Error fetching block {}", block_num);
            }
            
            if let Some(txs) = block.transactions.as_transactions() {
                // println!("Full transactions: {} found", txs.len());
                for (i, tx) in txs.iter().enumerate() {
                    // println!("  Tx {}: hash={:?}, gas_price={:?}", i, tx.tx_hash(), Transaction::gas_price(&tx.inner).unwrap_or_default());
                }
            } else {
                // println!("Transaction hashes only (no details available)");
            }
            // println!("==================");

            let txs = block.transactions.as_transactions().ok_or_else(|| {
                GasEstimatorError::CustomError("Failed to get transactions".to_string())
            })?.to_vec(); // Clone the transactions

            // println!("Extracted {} transactions from block", txs.len());

            for tx in txs {
                match (tx.max_priority_fee_per_gas(), Transaction::max_fee_per_gas(&tx), Transaction::gas_price(&tx)) {
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
