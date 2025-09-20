use std::{collections::HashMap, sync::Arc};

use super::fee_estimator::base::{GasEstimatorResult, GasPriceResult};
use crate::{
    network::types::ChainId, provider::EvmProvider, rrelayer_error, rrelayer_info,
    transaction::types::TransactionSpeed,
};
use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tracing::info;

/// Cache for gas prices across different chains.
///
/// Stores gas estimation results by chain ID and provides thread-safe access
/// using a Mutex to prevent race conditions during concurrent updates.
pub struct GasOracleCache {
    gas_prices: Mutex<HashMap<ChainId, GasEstimatorResult>>,
}

impl GasOracleCache {
    /// Creates a new empty gas oracle cache.
    ///
    /// # Returns
    /// * A new `GasOracleCache` instance with an empty hash map
    pub fn new() -> Self {
        GasOracleCache { gas_prices: Mutex::new(HashMap::new()) }
    }

    /// Updates the cached gas price for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to update gas prices for
    /// * `gas_price` - The new gas estimation result to cache
    async fn update_gas_price(&self, chain_id: ChainId, gas_price: GasEstimatorResult) {
        let mut cache = self.gas_prices.lock().await;
        cache.insert(chain_id, gas_price);
    }

    /// Retrieves the cached gas price for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to retrieve gas prices for
    ///
    /// # Returns
    /// * `Some(GasEstimatorResult)` - The cached gas estimation if available
    /// * `None` - If no gas prices are cached for this chain
    pub async fn get_gas_price(&self, chain_id: &ChainId) -> Option<GasEstimatorResult> {
        let cache = self.gas_prices.lock().await;
        cache.get(chain_id).cloned()
    }

    /// Retrieves the gas price for a specific chain and transaction speed.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to retrieve gas prices for
    /// * `speed` - The desired transaction speed (Super, Fast, Medium, or Slow)
    ///
    /// # Returns
    /// * `Some(GasPriceResult)` - The gas price for the specified speed if available
    /// * `None` - If no gas prices are cached for this chain
    pub async fn get_gas_price_for_speed(
        &self,
        chain_id: &ChainId,
        speed: &TransactionSpeed,
    ) -> Option<GasPriceResult> {
        let gas_prices = self.get_gas_price(chain_id).await?;

        match speed {
            TransactionSpeed::Super => Some(gas_prices.super_fast),
            TransactionSpeed::Fast => Some(gas_prices.fast),
            TransactionSpeed::Medium => Some(gas_prices.medium),
            TransactionSpeed::Slow => Some(gas_prices.slow),
        }
    }
}

/// Main gas oracle function that manages gas price collection and updates.
///
/// Initializes gas prices for all providers, then starts periodic updates
/// every 10 seconds. Handles errors gracefully by logging and continuing
/// with the next update cycle.
///
/// # Arguments
/// * `providers` - Vector of EVM providers to collect gas prices from
/// * `gas_oracle_cache` - Shared cache to store gas price results
pub async fn gas_oracle(
    providers: Arc<Vec<EvmProvider>>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
) {
    let mut initial_tasks = Vec::new();

    for provider in providers.iter() {
        info!("Getting initial gas price for provider: {}", provider.name);
        let cache = Arc::clone(&gas_oracle_cache);
        let provider = provider.clone();

        let task = tokio::spawn(async move {
            let gas_price_result = provider.calculate_gas_price().await;
            match gas_price_result {
                Ok(gas_price) => {
                    cache.lock().await.update_gas_price(provider.chain_id, gas_price).await;
                }
                Err(err) => {
                    rrelayer_error!(
                        "Failed to get initial gas price for provider: {} - error {}",
                        provider.name,
                        err
                    );
                }
            }
        });

        initial_tasks.push(task);
    }

    for task in initial_tasks {
        let _ = task.await;
    }

    rrelayer_info!("Initial gas price collection completed for all providers");

    for provider in providers.iter() {
        rrelayer_info!("Starting gas_oracle interval for provider: {}", provider.name);
        let cache = Arc::clone(&gas_oracle_cache);
        let provider = Arc::new(provider.clone());

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                let gas_price_result = provider.calculate_gas_price().await;
                match gas_price_result {
                    Ok(gas_price) => {
                        cache.lock().await.update_gas_price(provider.chain_id, gas_price).await;
                    }
                    Err(err) => {
                        rrelayer_error!("Failed to get gas price for provider: {} - error {} - try again in 10s", provider.name, err);
                    }
                }
            }
        });
    }
}
