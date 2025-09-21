mod automatic_top_up_task;
mod user_rate_limit_cleanup_task;
mod webhook_manager_task;

use crate::{
    background_tasks::{
        automatic_top_up_task::run_automatic_top_up_task,
        user_rate_limit_cleanup_task::run_user_rate_limit_cleanup_task,
        webhook_manager_task::run_webhook_manager_task,
    },
    gas::{
        blob_gas_oracle::{blob_gas_oracle, BlobGasOracleCache},
        gas_oracle::{gas_oracle, GasOracleCache},
    },
    provider::EvmProvider,
    rate_limiting::RateLimiter,
    rrelayer_info,
    webhooks::WebhookManager,
    PostgresClient, SetupConfig,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn run_background_tasks(
    config: &SetupConfig,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    postgres_client: Arc<PostgresClient>,
    user_rate_limiter: Option<Arc<RateLimiter>>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
) {
    rrelayer_info!("Starting background tasks");

    let gas_oracle_task = gas_oracle(Arc::clone(&providers), gas_oracle_cache);
    let blob_gas_oracle_task = blob_gas_oracle(Arc::clone(&providers), blob_gas_oracle_cache);
    let top_up_task = run_automatic_top_up_task(config.clone(), postgres_client, providers.clone());

    // Start webhook manager background tasks if enabled
    if let Some(webhook_manager) = webhook_manager {
        run_webhook_manager_task(webhook_manager, providers.clone()).await;
    }

    // Start rate limiter cleanup task if rate limiter is enabled
    if let Some(user_rate_limiter) = user_rate_limiter {
        run_user_rate_limit_cleanup_task(user_rate_limiter).await;
    }

    tokio::join!(gas_oracle_task, blob_gas_oracle_task, top_up_task);

    rrelayer_info!("Background tasks spawned up");
}
