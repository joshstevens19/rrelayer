use crate::{provider::EvmProvider, webhooks::WebhookManager};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub async fn run_webhook_manager_task(
    webhook_manager: Arc<Mutex<WebhookManager>>,
    providers: Arc<Vec<EvmProvider>>,
) {
    info!("Starting webhook manager background task");

    {
        let manager = webhook_manager.lock().await;
        let network_mappings: Vec<_> =
            providers.iter().map(|p| (p.chain_id, p.name.clone())).collect();
        manager.update_network_names(&network_mappings).await;
    }

    let manager_clone = webhook_manager.clone();
    tokio::spawn(async move {
        run_webhook_background_tasks(manager_clone).await;
    });

    info!("Started webhook manager background task");
}

/// Run webhook background tasks without holding the mutex indefinitely
async fn run_webhook_background_tasks(webhook_manager: Arc<Mutex<WebhookManager>>) {
    use tokio::time::{interval, Duration};

    let mut retry_interval = interval(Duration::from_secs(30));
    let mut cleanup_interval = interval(Duration::from_secs(300));
    let mut database_cleanup_interval = interval(Duration::from_secs(3600));

    info!("Starting webhook background processing loops");

    loop {
        tokio::select! {
            _ = retry_interval.tick() => {
                if let Ok(manager) = webhook_manager.try_lock() {
                    let ready_count = {
                        let pending = manager.pending_deliveries.read().await;
                        let now = std::time::SystemTime::now();
                        pending.values().filter(|d| d.is_ready_for_retry(now)).count()
                    };

                    if ready_count > 0 {
                        info!("Processing {} ready webhook deliveries", ready_count);
                        drop(manager);

                        process_ready_webhook_deliveries(webhook_manager.clone()).await;
                    }
                }
            }
            _ = cleanup_interval.tick() => {
                if let Ok(manager) = webhook_manager.try_lock() {
                    let initial_count = {
                        let pending = manager.pending_deliveries.read().await;
                        pending.len()
                    };

                    if initial_count > 0 {
                        drop(manager);
                        cleanup_webhook_deliveries(webhook_manager.clone()).await;
                    }
                }
            }
            _ = database_cleanup_interval.tick() => {
                cleanup_webhook_database_history(webhook_manager.clone()).await;
            }
        }
    }
}

async fn process_ready_webhook_deliveries(webhook_manager: Arc<Mutex<WebhookManager>>) {
    let ready_deliveries = {
        if let Ok(manager) = webhook_manager.try_lock() {
            let pending = manager.pending_deliveries.read().await;
            let now = std::time::SystemTime::now();
            pending.values().filter(|d| d.is_ready_for_retry(now)).cloned().collect::<Vec<_>>()
        } else {
            return;
        }
    };

    if ready_deliveries.is_empty() {
        return;
    }

    let updated_deliveries = {
        if let Ok(manager) = webhook_manager.try_lock() {
            manager.sender.send_multiple_webhooks(ready_deliveries).await
        } else {
            return;
        }
    };

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
            info!("Cleaned up {} completed/failed webhook deliveries", removed_count);
        }
    }
}

/// Cleanup old webhook delivery history from database
async fn cleanup_webhook_database_history(webhook_manager: Arc<Mutex<WebhookManager>>) {
    if let Ok(manager) = webhook_manager.try_lock() {
        let db = manager.sender.db.clone();
        drop(manager);

        match db.cleanup_old_webhook_deliveries().await {
            Ok(deleted_count) => {
                if deleted_count > 0 {
                    info!(
                        "Cleaned up {} old webhook delivery records from database",
                        deleted_count
                    );
                }
            }
            Err(e) => {
                info!("Failed to cleanup old webhook delivery records: {}", e);
            }
        }
    }
}
