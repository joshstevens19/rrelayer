mod automatic_top_up_task;
mod webhook_manager_task;

use crate::background_tasks::webhook_manager_task::run_webhook_manager_task;
use crate::gas::{blob_gas_oracle, gas_oracle, BlobGasOracleCache, GasOracleCache};
use crate::{
    background_tasks::automatic_top_up_task::run_automatic_top_up_task, provider::EvmProvider,
    shared::cache::Cache, transaction::queue_system::TransactionsQueues, webhooks::WebhookManager,
    PostgresClient, SafeProxyManager, SetupConfig,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub async fn run_background_tasks(
    config: &SetupConfig,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    postgres_client: Arc<PostgresClient>,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    safe_proxy_manager: Arc<SafeProxyManager>,
) {
    info!("Starting background tasks");

    let gas_oracle_task = gas_oracle(Arc::clone(&providers), gas_oracle_cache);

    // Only start blob gas oracle if any network has blobs enabled
    let has_blob_enabled_networks =
        config.networks.iter().any(|network| network.enable_sending_blobs.unwrap_or(false));

    let blob_gas_oracle_task = if has_blob_enabled_networks {
        info!("Starting blob gas oracle (blobs enabled on at least one network)");
        Some(blob_gas_oracle(Arc::clone(&providers), blob_gas_oracle_cache))
    } else {
        info!("Skipping blob gas oracle (no networks have blobs enabled)");
        None
    };
    let top_up_task = run_automatic_top_up_task(
        config.clone(),
        postgres_client.clone(),
        providers.clone(),
        transactions_queues,
        safe_proxy_manager,
    );

    if let Some(webhook_manager) = webhook_manager {
        run_webhook_manager_task(webhook_manager, providers.clone()).await;
    }

    // Start background tasks, conditionally starting blob gas oracle
    if let Some(blob_task) = blob_gas_oracle_task {
        tokio::join!(gas_oracle_task, blob_task, top_up_task);
    } else {
        tokio::join!(gas_oracle_task, top_up_task);
    }

    info!("Background tasks spawned up");
}
