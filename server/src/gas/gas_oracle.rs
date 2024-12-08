use std::{collections::HashMap, sync::Arc};

use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tracing::info;

use super::fee_estimator::base::{GasEstimatorResult, GasPriceResult};
use crate::{network::types::ChainId, provider::EvmProvider, transaction::types::TransactionSpeed};

// could use generic cache and kill code?
pub struct GasOracleCache {
    gas_prices: Mutex<HashMap<ChainId, GasEstimatorResult>>,
}

impl GasOracleCache {
    pub fn new() -> Self {
        GasOracleCache { gas_prices: Mutex::new(HashMap::new()) }
    }

    async fn update_gas_price(&self, chain_id: ChainId, gas_price: GasEstimatorResult) {
        let mut cache = self.gas_prices.lock().await;
        cache.insert(chain_id, gas_price);
    }

    pub async fn get_gas_price(&self, chain_id: &ChainId) -> Option<GasEstimatorResult> {
        let cache = self.gas_prices.lock().await;
        cache.get(chain_id).cloned()
    }

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

pub async fn gas_oracle(
    providers: Arc<Vec<EvmProvider>>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
) {
    for provider in providers.iter() {
        info!("Running gas_oracle cron for provider: {}", provider.name);
        let cache = Arc::clone(&gas_oracle_cache);
        let provider = Arc::new(provider.clone());

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10)); // Update every 10 seconds
            loop {
                let gas_price_result = provider.calculate_gas_price().await;

                if let Ok(gas_price) = gas_price_result {
                    cache.lock().await.update_gas_price(provider.chain_id, gas_price).await;
                }

                interval.tick().await;
            }
        });
    }
}
