use std::{collections::HashMap, sync::Arc};

use crate::{
    network::types::ChainId, provider::EvmProvider, rrelayer_error, rrelayer_info,
    transaction::types::TransactionSpeed,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tracing::info;

/// Result structure for blob gas estimates
#[derive(Clone, Debug)]
pub struct BlobGasEstimatorResult {
    pub super_fast: BlobGasPriceResult,
    pub fast: BlobGasPriceResult,
    pub medium: BlobGasPriceResult,
    pub slow: BlobGasPriceResult,
    pub base_fee_per_blob_gas: u128,
    pub timestamp: u64,
}

/// Price data for a specific blob gas speed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlobGasPriceResult {
    pub blob_gas_price: u128,     // Price per blob gas unit
    pub total_fee_for_blob: u128, // Total fee for a standard blob (128KB)
}

/// Standard amount of blob gas consumed per blob (128KB).
pub const BLOB_GAS_PER_BLOB: u128 = 131_072;

/// Cache for blob gas prices across different chains.
///
/// Stores blob gas estimation results by chain ID and provides thread-safe access
/// using a Mutex to prevent race conditions during concurrent updates.
pub struct BlobGasOracleCache {
    blob_gas_prices: Mutex<HashMap<ChainId, BlobGasEstimatorResult>>,
}

impl BlobGasOracleCache {
    /// Creates a new empty blob gas oracle cache.
    ///
    /// # Returns
    /// * A new `BlobGasOracleCache` instance with an empty hash map
    pub fn new() -> Self {
        BlobGasOracleCache { blob_gas_prices: Mutex::new(HashMap::new()) }
    }

    /// Updates the cached blob gas price for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to update blob gas prices for
    /// * `blob_gas_price` - The new blob gas estimation result to cache
    async fn update_blob_gas_price(
        &self,
        chain_id: ChainId,
        blob_gas_price: BlobGasEstimatorResult,
    ) {
        let mut cache = self.blob_gas_prices.lock().await;
        cache.insert(chain_id, blob_gas_price);
    }

    /// Retrieves the cached blob gas price for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to retrieve blob gas prices for
    ///
    /// # Returns
    /// * `Some(BlobGasEstimatorResult)` - The cached blob gas estimation if available
    /// * `None` - If no blob gas prices are cached for this chain
    pub async fn get_blob_gas_price(&self, chain_id: &ChainId) -> Option<BlobGasEstimatorResult> {
        let cache = self.blob_gas_prices.lock().await;
        cache.get(chain_id).cloned()
    }

    /// Retrieves the blob gas price for a specific chain and transaction speed.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to retrieve blob gas prices for
    /// * `speed` - The desired transaction speed (Super, Fast, Medium, or Slow)
    ///
    /// # Returns
    /// * `Some(BlobGasPriceResult)` - The blob gas price for the specified speed if available
    /// * `None` - If no blob gas prices are cached for this chain
    pub async fn get_blob_gas_price_for_speed(
        &self,
        chain_id: &ChainId,
        speed: &TransactionSpeed,
    ) -> Option<BlobGasPriceResult> {
        let blob_gas_prices = self.get_blob_gas_price(chain_id).await?;
        info!("Blob gas prices: {:?}", blob_gas_prices);

        match speed {
            TransactionSpeed::Super => Some(blob_gas_prices.super_fast),
            TransactionSpeed::Fast => Some(blob_gas_prices.fast),
            TransactionSpeed::Medium => Some(blob_gas_prices.medium),
            TransactionSpeed::Slow => Some(blob_gas_prices.slow),
        }
    }

    /// Estimates the total cost for multiple blobs at a specific transaction speed.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to retrieve blob gas prices for
    /// * `speed` - The desired transaction speed (Super, Fast, Medium, or Slow)
    /// * `blob_count` - The number of blobs to estimate cost for
    ///
    /// # Returns
    /// * `Some(u128)` - The total estimated cost for all blobs if prices are available
    /// * `None` - If no blob gas prices are cached for this chain
    pub async fn estimate_multiple_blobs(
        &self,
        chain_id: &ChainId,
        speed: &TransactionSpeed,
        blob_count: u32,
    ) -> Option<u128> {
        let price = self.get_blob_gas_price_for_speed(chain_id, speed).await?;
        Some(price.total_fee_for_blob * blob_count as u128)
    }
}

/// Main blob gas oracle function that manages blob gas price collection and updates.
///
/// Initializes blob gas prices for providers that support blob transactions, then starts
/// periodic updates every 20 seconds. Only processes providers that support blob transactions
/// and handles errors gracefully by logging and continuing with the next update cycle.
///
/// # Arguments
/// * `providers` - Vector of EVM providers to collect blob gas prices from
/// * `blob_gas_oracle_cache` - Shared cache to store blob gas price results
pub async fn blob_gas_oracle(
    providers: Arc<Vec<EvmProvider>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
) {
    let mut initial_tasks = Vec::new();

    for provider in providers.iter() {
        if !provider.supports_blob_transactions() {
            continue;
        }

        rrelayer_info!("Getting initial blob gas price for provider: {}", provider.name);
        let cache = Arc::clone(&blob_gas_oracle_cache);
        let provider = provider.clone();

        let task = tokio::spawn(async move {
            let blob_gas_price_result = provider.calculate_ethereum_blob_gas_price().await;
            if let Ok(blob_gas_price) = blob_gas_price_result {
                cache.lock().await.update_blob_gas_price(provider.chain_id, blob_gas_price).await;
            } else {
                rrelayer_error!(
                    "Failed to get initial blob gas price for provider: {} - error {:?}",
                    provider.name,
                    blob_gas_price_result
                );
            }
        });

        initial_tasks.push(task);
    }

    for task in initial_tasks {
        let _ = task.await;
    }

    info!("Initial blob gas price collection completed for all blob-supporting providers");

    for provider in providers.iter() {
        if !provider.supports_blob_transactions() {
            continue;
        }

        info!("Starting blob_gas_oracle interval for provider: {}", provider.name);
        let cache = Arc::clone(&blob_gas_oracle_cache);
        let provider = Arc::new(provider.clone());

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(20));
            loop {
                interval.tick().await;

                let blob_gas_price_result = provider.calculate_ethereum_blob_gas_price().await;
                if let Ok(blob_gas_price) = blob_gas_price_result {
                    cache
                        .lock()
                        .await
                        .update_blob_gas_price(provider.chain_id, blob_gas_price)
                        .await;
                }
            }
        });
    }
}
