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
    }

    // Spawn background tasks without holding the lock
    let manager_clone = webhook_manager.clone();
    tokio::spawn(async move {
        run_webhook_background_tasks(manager_clone).await;
    });

    rrelayer_info!("Webhook manager background task initialized");
}

/// Run webhook background tasks without holding the mutex indefinitely
async fn run_webhook_background_tasks(webhook_manager: Arc<Mutex<WebhookManager>>) {
    use tokio::time::{interval, Duration};

    let mut retry_interval = interval(Duration::from_secs(30)); // 30 seconds
    let mut cleanup_interval = interval(Duration::from_secs(300)); // 5 minutes
    let mut database_cleanup_interval = interval(Duration::from_secs(3600)); // 1 hour

    rrelayer_info!("Starting webhook background processing loops");

    loop {
        tokio::select! {
            _ = retry_interval.tick() => {
                // Process ready deliveries without holding lock indefinitely
                if let Ok(manager) = webhook_manager.try_lock() {
                    // Use a temporary method that doesn't require &mut self
                    let ready_count = {
                        let pending = manager.pending_deliveries.read().await;
                        let now = std::time::SystemTime::now();
                        pending.values().filter(|d| d.is_ready_for_retry(now)).count()
                    };

                    if ready_count > 0 {
                        rrelayer_info!("Processing {} ready webhook deliveries", ready_count);
                        drop(manager); // Release lock before processing

                        // Process deliveries with temporary locks
                        process_ready_webhook_deliveries(webhook_manager.clone()).await;
                    }
                }
            }
            _ = cleanup_interval.tick() => {
                // Cleanup completed deliveries
                if let Ok(manager) = webhook_manager.try_lock() {
                    let initial_count = {
                        let pending = manager.pending_deliveries.read().await;
                        pending.len()
                    };

                    if initial_count > 0 {
                        drop(manager); // Release lock before cleanup
                        cleanup_webhook_deliveries(webhook_manager.clone()).await;
                    }
                }
            }
            _ = database_cleanup_interval.tick() => {
                // Cleanup old webhook delivery history from database
                cleanup_webhook_database_history(webhook_manager.clone()).await;
            }
        }
    }
}

/// Process ready webhook deliveries without holding manager mutex
async fn process_ready_webhook_deliveries(webhook_manager: Arc<Mutex<WebhookManager>>) {
    let ready_deliveries = {
        if let Ok(manager) = webhook_manager.try_lock() {
            let pending = manager.pending_deliveries.read().await;
            let now = std::time::SystemTime::now();
            pending.values().filter(|d| d.is_ready_for_retry(now)).cloned().collect::<Vec<_>>()
        } else {
            return; // Skip if we can't get lock
        }
    };

    if ready_deliveries.is_empty() {
        return;
    }

    // Send webhooks (this doesn't need the manager lock)
    let updated_deliveries = {
        if let Ok(manager) = webhook_manager.try_lock() {
            manager.sender.send_multiple_webhooks(ready_deliveries).await
        } else {
            return; // Skip if we can't get lock
        }
    };

    // Update delivery statuses
    if let Ok(manager) = webhook_manager.try_lock() {
        let mut pending = manager.pending_deliveries.write().await;
        for delivery in updated_deliveries {
            pending.insert(delivery.id, delivery);
        }
    }
}

/// Cleanup completed webhook deliveries
async fn cleanup_webhook_deliveries(webhook_manager: Arc<Mutex<WebhookManager>>) {
    if let Ok(manager) = webhook_manager.try_lock() {
        let mut pending = manager.pending_deliveries.write().await;
        let initial_count = pending.len();

        pending.retain(|_, delivery| !delivery.completed && !delivery.failed);

        let removed_count = initial_count - pending.len();
        if removed_count > 0 {
            rrelayer_info!("Cleaned up {} completed/failed webhook deliveries", removed_count);
        }
    }
}

/// Cleanup old webhook delivery history from database (30+ days old)
async fn cleanup_webhook_database_history(webhook_manager: Arc<Mutex<WebhookManager>>) {
    if let Ok(manager) = webhook_manager.try_lock() {
        // Access the database through the webhook sender
        let db = manager.sender.db.clone();
        drop(manager); // Release lock before database operation

        match db.cleanup_old_webhook_deliveries().await {
            Ok(deleted_count) => {
                if deleted_count > 0 {
                    rrelayer_info!(
                        "Cleaned up {} old webhook delivery records from database",
                        deleted_count
                    );
                }
            }
            Err(e) => {
                rrelayer_info!("Failed to cleanup old webhook delivery records: {}", e);
            }
        }
    }
}
