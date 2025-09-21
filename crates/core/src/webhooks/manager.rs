use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    sync::RwLock,
    time::{interval, Interval},
};
use tracing::debug;
use uuid::Uuid;

use crate::{
    network::types::ChainId, rrelayer_error, rrelayer_info, transaction::types::Transaction,
    yaml::WebhookConfig, SetupConfig,
};

use super::{
    payload::{WebhookPayload, WebhookSigningPayload},
    sender::WebhookSender,
    types::{WebhookDelivery, WebhookDeliveryConfig, WebhookEventType, WebhookFilter},
};

/// WebhookManager handles the lifecycle of webhook deliveries
/// including queuing, retries, and cleanup
pub struct WebhookManager {
    /// Active webhook deliveries pending retry or completion
    pub(crate) pending_deliveries: Arc<RwLock<HashMap<Uuid, WebhookDelivery>>>,
    /// Webhook sender for HTTP requests
    pub(crate) sender: WebhookSender,
    /// Configured webhooks from setup
    webhook_configs: Vec<WebhookConfig>,
    /// Network name mapping for filtering
    network_names: Arc<RwLock<HashMap<ChainId, String>>>,
    /// Cleanup interval for completed/failed deliveries
    cleanup_interval: Interval,
    /// Delivery retry interval
    retry_interval: Interval,
}

impl WebhookManager {
    /// Creates a new webhook manager from configuration.
    ///
    /// Initializes the webhook manager with delivery settings, HTTP client,
    /// and background task intervals for processing and cleanup.
    ///
    /// # Arguments
    /// * `config` - Application configuration containing webhook settings
    /// * `delivery_config` - Optional delivery configuration (uses defaults if None)
    ///
    /// # Returns
    /// * `Ok(WebhookManager)` - Configured webhook manager
    /// * `Err(reqwest::Error)` - If HTTP client creation fails
    pub fn new(
        config: &SetupConfig,
        delivery_config: Option<WebhookDeliveryConfig>,
    ) -> Result<Self, reqwest::Error> {
        let webhook_configs = config.webhooks.as_ref().cloned().unwrap_or_default();
        rrelayer_info!("🔧 WebhookManager::new - Found {} webhook configs", webhook_configs.len());
        for (i, config) in webhook_configs.iter().enumerate() {
            rrelayer_info!("  {}. Endpoint: {}, Networks: {:?}", i + 1, config.endpoint, config.networks);
        }
        
        let delivery_config = delivery_config.unwrap_or_default();
        let sender = WebhookSender::new(delivery_config)?;

        // Build network name mapping
        let mut network_names = HashMap::new();
        for network in &config.networks {
            // Try to get chain_id synchronously or use a placeholder
            // In practice, you might want to populate this asynchronously
            if let Ok(chain_id) = network.name.parse::<u64>() {
                network_names.insert(ChainId::new(chain_id), network.name.clone());
            }
            // For now, we'll handle this in a separate method
        }

        Ok(Self {
            pending_deliveries: Arc::new(RwLock::new(HashMap::new())),
            sender,
            webhook_configs,
            network_names: Arc::new(RwLock::new(network_names)),
            cleanup_interval: interval(Duration::from_secs(300)), // 5 minutes
            retry_interval: interval(Duration::from_secs(30)),    // 30 seconds
        })
    }

    /// Update network name mapping (call this after network initialization)
    pub async fn update_network_names(&self, networks: &[(ChainId, String)]) {
        let mut names = self.network_names.write().await;
        for (chain_id, name) in networks {
            names.insert(*chain_id, name.clone());
        }
        rrelayer_info!("Updated webhook network names for {} networks", networks.len());
    }

    /// Queue a webhook for a transaction event with custom payload
    pub async fn queue_webhook_with_payload(
        &self,
        transaction: &Transaction,
        payload: WebhookPayload,
    ) {
        if self.webhook_configs.is_empty() {
            rrelayer_info!(
                "No webhooks configured, skipping webhook for transaction {}",
                transaction.id
            );
            return;
        }

        let network_names = self.network_names.read().await;
        let chain_name = network_names
            .get(&transaction.chain_id)
            .cloned()
            .unwrap_or_else(|| transaction.chain_id.to_string());

        let payload_json = match payload.to_json_value() {
            Ok(json) => json,
            Err(e) => {
                rrelayer_error!(
                    "Failed to serialize webhook payload for transaction {}: {}",
                    transaction.id,
                    e
                );
                return;
            }
        };

        let mut deliveries_to_queue = Vec::new();

        for webhook_config in &self.webhook_configs {
            if WebhookFilter::should_send_webhook(webhook_config, transaction, &chain_name) {
                let delivery = WebhookDelivery::new(
                    webhook_config.clone(),
                    payload.event_type.clone(),
                    payload_json.clone(),
                    3, // Default max retries, could be configurable per webhook
                );
                deliveries_to_queue.push(delivery);
            }
        }

        if deliveries_to_queue.is_empty() {
            debug!(
                "No webhooks matched filters for transaction {} on chain {}",
                transaction.id, chain_name
            );
            return;
        }

        rrelayer_info!(
            "Queuing {} webhooks for transaction {} event {} on chain {}",
            deliveries_to_queue.len(),
            transaction.id,
            serde_json::to_string(&payload.event_type).unwrap_or_default(),
            chain_name
        );

        // Add to pending deliveries
        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        // Trigger immediate processing for fresh webhooks
        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

    /// Queue a webhook for a signing event
    pub async fn queue_signing_webhook(
        &self,
        relayer_id: &crate::relayer::types::RelayerId,
        chain_id: ChainId,
        payload: WebhookSigningPayload,
    ) {
        if self.webhook_configs.is_empty() {
            rrelayer_info!(
                "No webhooks configured, skipping signing webhook for relayer {}",
                relayer_id
            );
            return;
        }

        let network_names = self.network_names.read().await;
        let chain_name = network_names
            .get(&chain_id)
            .cloned()
            .unwrap_or_else(|| chain_id.to_string());

        let payload_json = match payload.to_json_value() {
            Ok(json) => json,
            Err(e) => {
                rrelayer_error!(
                    "Failed to serialize signing webhook payload for relayer {}: {}",
                    relayer_id,
                    e
                );
                return;
            }
        };

        let mut deliveries_to_queue = Vec::new();

        for webhook_config in &self.webhook_configs {
            // Check if webhook should receive events for this network
            if webhook_config.networks.is_empty()
                || webhook_config.networks.contains(&chain_name)
                || webhook_config.networks.contains(&"*".to_string())
            {
                let delivery = WebhookDelivery::new(
                    webhook_config.clone(),
                    payload.event_type.clone(),
                    payload_json.clone(),
                    3, // Default max retries
                );
                deliveries_to_queue.push(delivery);
            }
        }

        if deliveries_to_queue.is_empty() {
            debug!(
                "No webhooks matched filters for signing operation relayer {} on chain {}",
                relayer_id, chain_name
            );
            return;
        }

        rrelayer_info!(
            "Queuing {} signing webhooks for relayer {} event {} on chain {}",
            deliveries_to_queue.len(),
            relayer_id,
            serde_json::to_string(&payload.event_type).unwrap_or_default(),
            chain_name
        );

        // Add to pending deliveries
        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        // Trigger immediate processing for fresh webhooks
        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

    /// Queue a webhook for a transaction event
    pub async fn queue_webhook(&self, transaction: &Transaction, event_type: WebhookEventType) {
        rrelayer_info!("🔔 queue_webhook called for transaction {} with event {:?}", transaction.id, event_type);
        
        if self.webhook_configs.is_empty() {
            rrelayer_info!(
                "❌ No webhooks configured, skipping webhook for transaction {}",
                transaction.id
            );
            return;
        }
        
        rrelayer_info!("✅ Found {} webhook configs for transaction {}", self.webhook_configs.len(), transaction.id);

        let network_names = self.network_names.read().await;
        let chain_name = network_names
            .get(&transaction.chain_id)
            .cloned()
            .unwrap_or_else(|| transaction.chain_id.to_string());

        let payload = WebhookPayload::new(transaction, event_type.clone());
        let payload_json = match payload.to_json_value() {
            Ok(json) => json,
            Err(e) => {
                rrelayer_error!(
                    "Failed to serialize webhook payload for transaction {}: {}",
                    transaction.id,
                    e
                );
                return;
            }
        };

        let mut deliveries_to_queue = Vec::new();

        for webhook_config in &self.webhook_configs {
            if WebhookFilter::should_send_webhook(webhook_config, transaction, &chain_name) {
                let delivery = WebhookDelivery::new(
                    webhook_config.clone(),
                    event_type.clone(),
                    payload_json.clone(),
                    3, // Default max retries, could be configurable per webhook
                );
                deliveries_to_queue.push(delivery);
            }
        }

        if deliveries_to_queue.is_empty() {
            debug!(
                "No webhooks matched filters for transaction {} on chain {}",
                transaction.id, chain_name
            );
            return;
        }

        rrelayer_info!(
            "Queuing {} webhooks for transaction {} event {} on chain {}",
            deliveries_to_queue.len(),
            transaction.id,
            serde_json::to_string(&event_type).unwrap_or_default(),
            chain_name
        );

        // Add to pending deliveries
        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        // Trigger immediate processing for fresh webhooks
        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

    /// Process deliveries that are ready for sending/retry
    async fn process_ready_deliveries(&self) {
        let now = SystemTime::now();
        let ready_deliveries = {
            let pending = self.pending_deliveries.read().await;
            pending.values().filter(|d| d.is_ready_for_retry(now)).cloned().collect::<Vec<_>>()
        };

        if ready_deliveries.is_empty() {
            debug!("No webhook deliveries ready for processing");
            return;
        }

        rrelayer_info!("Processing {} ready webhook deliveries", ready_deliveries.len());

        // Send webhooks concurrently
        let updated_deliveries = self.sender.send_multiple_webhooks(ready_deliveries).await;

        // Update delivery statuses
        let mut pending = self.pending_deliveries.write().await;
        for delivery in updated_deliveries {
            pending.insert(delivery.id, delivery);
        }
    }

    /// Clean up completed and permanently failed deliveries
    async fn cleanup_deliveries(&self) {
        let mut pending = self.pending_deliveries.write().await;
        let initial_count = pending.len();

        pending.retain(|_, delivery| !delivery.completed && !delivery.failed);

        let removed_count = initial_count - pending.len();
        if removed_count > 0 {
            rrelayer_info!("Cleaned up {} completed/failed webhook deliveries", removed_count);
        }
    }

    /// Get statistics about pending deliveries
    pub async fn get_stats(&self) -> WebhookManagerStats {
        let pending = self.pending_deliveries.read().await;
        let mut stats = WebhookManagerStats::default();

        for delivery in pending.values() {
            stats.total += 1;
            if delivery.completed {
                stats.completed += 1;
            } else if delivery.failed {
                stats.failed += 1;
            } else {
                stats.pending += 1;
                if delivery.attempts > 0 {
                    stats.retrying += 1;
                }
            }
        }

        stats
    }

    /// Run the webhook manager background tasks
    pub async fn run_background_tasks(&mut self) {
        rrelayer_info!("Starting webhook manager background tasks");

        loop {
            tokio::select! {
                _ = self.retry_interval.tick() => {
                    self.process_ready_deliveries().await;
                }
                _ = self.cleanup_interval.tick() => {
                    self.cleanup_deliveries().await;
                }
            }
        }
    }

    /// Manually trigger webhook for transaction (useful for testing)
    pub async fn trigger_webhook_for_transaction(&self, transaction: &Transaction) {
        let event_type = WebhookEventType::from(transaction.status);
        self.queue_webhook(transaction, event_type).await;
    }

    /// Get count of pending deliveries
    pub async fn pending_count(&self) -> usize {
        self.pending_deliveries.read().await.len()
    }

    /// Check if any webhooks are configured
    pub fn has_webhooks(&self) -> bool {
        !self.webhook_configs.is_empty()
    }
}

impl Clone for WebhookManager {
    fn clone(&self) -> Self {
        Self {
            pending_deliveries: self.pending_deliveries.clone(),
            sender: self.sender.clone(),
            webhook_configs: self.webhook_configs.clone(),
            network_names: self.network_names.clone(),
            cleanup_interval: interval(Duration::from_secs(300)),
            retry_interval: interval(Duration::from_secs(30)),
        }
    }
}

/// Statistics about webhook manager state
#[derive(Debug, Default)]
pub struct WebhookManagerStats {
    pub total: usize,
    pub pending: usize,
    pub completed: usize,
    pub failed: usize,
    pub retrying: usize,
}

/// Convenience functions for common webhook events
impl WebhookManager {
    /// Fire webhook when transaction is queued
    pub async fn on_transaction_queued(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionQueued).await;
    }

    /// Fire webhook when transaction is sent
    pub async fn on_transaction_sent(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionSent).await;
    }

    /// Fire webhook when transaction is mined
    pub async fn on_transaction_mined(
        &self,
        transaction: &Transaction,
        receipt: &alloy::network::AnyTransactionReceipt,
    ) {
        let payload = WebhookPayload::transaction_mined_with_receipt(transaction, receipt);
        self.queue_webhook_with_payload(transaction, payload).await;
    }

    /// Fire webhook when transaction is confirmed
    pub async fn on_transaction_confirmed(
        &self,
        transaction: &Transaction,
        receipt: &alloy::network::AnyTransactionReceipt,
    ) {
        let payload = WebhookPayload::transaction_confirmed_with_receipt(transaction, receipt);
        self.queue_webhook_with_payload(transaction, payload).await;
    }

    /// Fire webhook when transaction fails
    pub async fn on_transaction_failed(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionFailed).await;
    }

    /// Fire webhook when transaction expires
    pub async fn on_transaction_expired(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionExpired).await;
    }

    /// Fire webhook when transaction is cancelled
    pub async fn on_transaction_cancelled(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionCancelled).await;
    }

    /// Fire webhook when transaction is replaced
    pub async fn on_transaction_replaced(
        &self,
        new_transaction: &Transaction,
        original_transaction: &Transaction,
    ) {
        let payload = WebhookPayload::transaction_replaced(new_transaction, original_transaction);
        self.queue_webhook_with_payload(new_transaction, payload).await;
    }

    /// Fire webhook when text is signed
    pub async fn on_text_signed(
        &self,
        relayer_id: &crate::relayer::types::RelayerId,
        chain_id: ChainId,
        message: String,
        signature: alloy::primitives::PrimitiveSignature,
    ) {
        let payload = WebhookSigningPayload::text_signed(
            relayer_id.clone(),
            chain_id,
            message,
            signature,
        );
        self.queue_signing_webhook(relayer_id, chain_id, payload).await;
    }

    /// Fire webhook when typed data is signed
    pub async fn on_typed_data_signed(
        &self,
        relayer_id: &crate::relayer::types::RelayerId,
        chain_id: ChainId,
        domain_data: serde_json::Value,
        message_data: serde_json::Value,
        primary_type: String,
        signature: alloy::primitives::PrimitiveSignature,
    ) {
        let payload = WebhookSigningPayload::typed_data_signed(
            relayer_id.clone(),
            chain_id,
            domain_data,
            message_data,
            primary_type,
            signature,
        );
        self.queue_signing_webhook(relayer_id, chain_id, payload).await;
    }
}
