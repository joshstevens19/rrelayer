use crate::{provider::EvmProvider, rrelayer_info, webhooks::WebhookManager};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Runs the webhook manager background task.
///
/// This function initializes the webhook manager with network mappings
/// and starts its background processing tasks.
///
/// # Arguments
/// * `webhook_manager` - The webhook manager instance
/// * `providers` - EVM providers for network mapping
pub async fn run_webhook_manager_task(
    webhook_manager: Arc<Mutex<WebhookManager>>,
    providers: Arc<Vec<EvmProvider>>,
) {
    rrelayer_info!("Starting webhook manager background task");

    // Initialize webhook manager
    {
        let manager = webhook_manager.lock().await;
        // Update network name mappings
        let network_mappings: Vec<_> =
            providers.iter().map(|p| (p.chain_id, p.name.clone())).collect();
        manager.update_network_names(&network_mappings).await;

        // Spawn background tasks
        let manager_clone = webhook_manager.clone();
        tokio::spawn(async move {
            let mut manager = manager_clone.lock().await;
            manager.run_background_tasks().await;
        });
    }

    rrelayer_info!("Webhook manager background task initialized");
}
