use std::{collections::HashMap, sync::Arc};

use super::fee_estimator::{GasEstimatorResult, GasPriceResult};
use crate::{network::ChainId, provider::EvmProvider, transaction::types::TransactionSpeed};
use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tracing::{error, info};

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
            TransactionSpeed::SUPER => Some(gas_prices.super_fast),
            TransactionSpeed::FAST => Some(gas_prices.fast),
            TransactionSpeed::MEDIUM => Some(gas_prices.medium),
            TransactionSpeed::SLOW => Some(gas_prices.slow),
        }
    }
}

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
                    error!(
                        "Failed to get initial gas price for provider: {} - error {}",
                        provider.name, err
                    );
                }
            }
        });

        initial_tasks.push(task);
    }

    for task in initial_tasks {
        let _ = task.await;
    }

    info!("Initial gas price collection completed for all providers");

    for provider in providers.iter() {
        info!("Starting gas_oracle interval for provider: {}", provider.name);
        let cache = Arc::clone(&gas_oracle_cache);
        let provider = Arc::new(provider.clone());

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;

                let gas_price_result = provider.calculate_gas_price().await;
                match gas_price_result {
                    Ok(gas_price) => {
                        cache.lock().await.update_gas_price(provider.chain_id, gas_price).await;
                    }
                    Err(err) => {
                        error!("Failed to get gas price for provider: {} - error {} - try again in 10s", provider.name, err);
                    }
                }
            }
        });
    }

    info!("gas_oracle interval started for all providers");
}
