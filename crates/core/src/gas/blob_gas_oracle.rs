use std::{collections::HashMap, sync::Arc};

use crate::{network::ChainId, provider::EvmProvider, transaction::types::TransactionSpeed};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tracing::{error, info};

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
    pub blob_gas_price: u128,
    pub total_fee_for_blob: u128,
}

pub const BLOB_GAS_PER_BLOB: u128 = 131_072;

pub struct BlobGasOracleCache {
    blob_gas_prices: Mutex<HashMap<ChainId, BlobGasEstimatorResult>>,
}

impl BlobGasOracleCache {
    pub fn new() -> Self {
        BlobGasOracleCache { blob_gas_prices: Mutex::new(HashMap::new()) }
    }

    async fn update_blob_gas_price(
        &self,
        chain_id: ChainId,
        blob_gas_price: BlobGasEstimatorResult,
    ) {
        let mut cache = self.blob_gas_prices.lock().await;
        cache.insert(chain_id, blob_gas_price);
    }

    pub async fn get_blob_gas_price(&self, chain_id: &ChainId) -> Option<BlobGasEstimatorResult> {
        let cache = self.blob_gas_prices.lock().await;
        cache.get(chain_id).cloned()
    }

    pub async fn get_blob_gas_price_for_speed(
        &self,
        chain_id: &ChainId,
        speed: &TransactionSpeed,
    ) -> Option<BlobGasPriceResult> {
        let blob_gas_prices = self.get_blob_gas_price(chain_id).await?;
        info!("Blob gas prices: {:?}", blob_gas_prices);

        match speed {
            TransactionSpeed::SUPER => Some(blob_gas_prices.super_fast),
            TransactionSpeed::FAST => Some(blob_gas_prices.fast),
            TransactionSpeed::MEDIUM => Some(blob_gas_prices.medium),
            TransactionSpeed::SLOW => Some(blob_gas_prices.slow),
        }
    }

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

pub async fn blob_gas_oracle(
    providers: Arc<Vec<EvmProvider>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
) {
    let mut initial_tasks = Vec::new();

    for provider in providers.iter() {
        if !provider.supports_blob_transactions() {
            continue;
        }

        info!("Getting initial blob gas price for provider: {}", provider.name);
        let cache = Arc::clone(&blob_gas_oracle_cache);
        let provider = provider.clone();

        let task = tokio::spawn(async move {
            let blob_gas_price_result = provider.calculate_ethereum_blob_gas_price().await;
            if let Ok(blob_gas_price) = blob_gas_price_result {
                cache.lock().await.update_blob_gas_price(provider.chain_id, blob_gas_price).await;
            } else {
                error!(
                    "Failed to get initial blob gas price for provider: {} - error {:?}",
                    provider.name, blob_gas_price_result
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
            let mut interval = time::interval(Duration::from_secs(5));
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
