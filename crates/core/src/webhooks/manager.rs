use alloy::primitives::U256;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    sync::RwLock,
    time::{interval, Interval},
};
use tracing::{debug, error, info};
use uuid::Uuid;

use super::{
    payload::{WebhookPayload, WebhookSigningPayload},
    sender::WebhookSender,
    types::{WebhookDelivery, WebhookDeliveryConfig, WebhookEventType, WebhookFilter},
};
use crate::relayer::RelayerId;
use crate::{
    network::ChainId, postgres::PostgresClient, transaction::types::Transaction,
    yaml::WebhookConfig, SetupConfig,
};

pub struct WebhookManager {
    pub pending_deliveries: Arc<RwLock<HashMap<Uuid, WebhookDelivery>>>,
    pub sender: WebhookSender,
    webhook_configs: Vec<WebhookConfig>,
    network_names: Arc<RwLock<HashMap<ChainId, String>>>,
    // TODO: REVIEW
    #[allow(dead_code)]
    cleanup_interval: Interval,
    // TODO: REVIEW
    #[allow(dead_code)]
    retry_interval: Interval,
}

impl WebhookManager {
    pub fn new(
        db: Arc<PostgresClient>,
        config: &SetupConfig,
        delivery_config: Option<WebhookDeliveryConfig>,
    ) -> Result<Self, reqwest::Error> {
        let webhook_configs = config.webhooks.as_ref().cloned().unwrap_or_default();
        info!("WebhookManager::new - Found {} webhook configs", webhook_configs.len());
        for (i, config) in webhook_configs.iter().enumerate() {
            info!("  {}. Endpoint: {}, Networks: {:?}", i + 1, config.endpoint, config.networks);
        }

        let delivery_config = delivery_config.unwrap_or_default();
        let sender = WebhookSender::new(delivery_config, db)?;

        let mut network_names = HashMap::new();
        for network in &config.networks {
            if let Ok(chain_id) = network.name.parse::<u64>() {
                network_names.insert(ChainId::new(chain_id), network.name.clone());
            }
        }

        Ok(Self {
            pending_deliveries: Arc::new(RwLock::new(HashMap::new())),
            sender,
            webhook_configs,
            network_names: Arc::new(RwLock::new(network_names)),
            cleanup_interval: interval(Duration::from_secs(300)),
            retry_interval: interval(Duration::from_secs(30)),
        })
    }

    pub async fn update_network_names(&self, networks: &[(ChainId, String)]) {
        let mut names = self.network_names.write().await;
        for (chain_id, name) in networks {
            names.insert(*chain_id, name.clone());
        }
        info!("Updated webhook network names for {} networks", networks.len());
    }

    pub async fn queue_webhook_with_payload(
        &self,
        transaction: &Transaction,
        payload: WebhookPayload,
    ) {
        if self.webhook_configs.is_empty() {
            info!("No webhooks configured, skipping webhook for transaction {}", transaction.id);
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
                error!(
                    "Failed to serialize webhook payload for transaction {}: {}",
                    transaction.id, e
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

        info!(
            "Queuing {} webhooks for transaction {} event {} on chain {}",
            deliveries_to_queue.len(),
            transaction.id,
            serde_json::to_string(&payload.event_type).unwrap_or_default(),
            chain_name
        );

        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

    pub async fn queue_signing_webhook(
        &self,
        relayer_id: &RelayerId,
        chain_id: ChainId,
        payload: WebhookSigningPayload,
    ) {
        if self.webhook_configs.is_empty() {
            info!("No webhooks configured, skipping signing webhook for relayer {}", relayer_id);
            return;
        }

        let network_names = self.network_names.read().await;
        let chain_name =
            network_names.get(&chain_id).cloned().unwrap_or_else(|| chain_id.to_string());

        let payload_json = match payload.to_json_value() {
            Ok(json) => json,
            Err(e) => {
                error!(
                    "Failed to serialize signing webhook payload for relayer {}: {}",
                    relayer_id, e
                );
                return;
            }
        };

        let mut deliveries_to_queue = Vec::new();

        for webhook_config in &self.webhook_configs {
            if webhook_config.networks.is_empty() || webhook_config.networks.contains(&chain_name) {
                let delivery = WebhookDelivery::new(
                    webhook_config.clone(),
                    payload.event_type.clone(),
                    payload_json.clone(),
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

        info!(
            "Queuing {} signing webhooks for relayer {} event {} on chain {}",
            deliveries_to_queue.len(),
            relayer_id,
            serde_json::to_string(&payload.event_type).unwrap_or_default(),
            chain_name
        );

        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

    pub async fn queue_webhook(&self, transaction: &Transaction, event_type: WebhookEventType) {
        info!(
            "queue_webhook called for transaction {} with event {:?}",
            transaction.id, event_type
        );

        if self.webhook_configs.is_empty() {
            info!("No webhooks configured, skipping webhook for transaction {}", transaction.id);
            return;
        }

        info!(
            "Found {} webhook configs for transaction {}",
            self.webhook_configs.len(),
            transaction.id
        );

        let network_names = self.network_names.read().await;
        let chain_name = network_names
            .get(&transaction.chain_id)
            .cloned()
            .unwrap_or_else(|| transaction.chain_id.to_string());

        let payload = WebhookPayload::new(transaction, event_type.clone());
        let payload_json = match payload.to_json_value() {
            Ok(json) => json,
            Err(e) => {
                error!(
                    "Failed to serialize webhook payload for transaction {}: {}",
                    transaction.id, e
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

        info!(
            "Queuing {} webhooks for transaction {} event {} on chain {}",
            deliveries_to_queue.len(),
            transaction.id,
            serde_json::to_string(&event_type).unwrap_or_default(),
            chain_name
        );

        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }

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

        info!("Processing {} ready webhook deliveries", ready_deliveries.len());

        let updated_deliveries = self.sender.send_multiple_webhooks(ready_deliveries).await;

        let mut pending = self.pending_deliveries.write().await;
        for delivery in updated_deliveries {
            pending.insert(delivery.id, delivery);
        }
    }

    // TODO: REVIEW
    #[allow(dead_code)]
    async fn cleanup_deliveries(&self) {
        let mut pending = self.pending_deliveries.write().await;
        let initial_count = pending.len();

        pending.retain(|_, delivery| !delivery.completed && !delivery.failed);

        let removed_count = initial_count - pending.len();
        if removed_count > 0 {
            info!("Cleaned up {} completed/failed webhook deliveries", removed_count);
        }
    }

    pub async fn pending_count(&self) -> usize {
        self.pending_deliveries.read().await.len()
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

impl WebhookManager {
    pub async fn on_transaction_queued(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionQueued).await;
    }

    pub async fn on_transaction_sent(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionSent).await;
    }

    pub async fn on_transaction_mined(
        &self,
        transaction: &Transaction,
        receipt: &alloy::network::AnyTransactionReceipt,
    ) {
        let payload = WebhookPayload::transaction_mined_with_receipt(transaction, receipt);
        self.queue_webhook_with_payload(transaction, payload).await;
    }

    pub async fn on_transaction_confirmed(
        &self,
        transaction: &Transaction,
        receipt: &alloy::network::AnyTransactionReceipt,
    ) {
        let payload = WebhookPayload::transaction_confirmed_with_receipt(transaction, receipt);
        self.queue_webhook_with_payload(transaction, payload).await;
    }

    pub async fn on_transaction_failed(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionFailed).await;
    }

    pub async fn on_transaction_expired(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionExpired).await;
    }

    pub async fn on_transaction_cancelled(&self, transaction: &Transaction) {
        self.queue_webhook(transaction, WebhookEventType::TransactionCancelled).await;
    }

    pub async fn on_transaction_replaced(
        &self,
        new_transaction: &Transaction,
        original_transaction: &Transaction,
    ) {
        let payload = WebhookPayload::transaction_replaced(new_transaction, original_transaction);
        self.queue_webhook_with_payload(new_transaction, payload).await;
    }

    pub async fn on_text_signed(
        &self,
        relayer_id: &RelayerId,
        chain_id: ChainId,
        message: String,
        signature: alloy::primitives::Signature,
    ) {
        let payload = WebhookSigningPayload::text_signed(*relayer_id, chain_id, message, signature);
        self.queue_signing_webhook(relayer_id, chain_id, payload).await;
    }

    pub async fn on_typed_data_signed(
        &self,
        relayer_id: &RelayerId,
        chain_id: ChainId,
        domain_data: serde_json::Value,
        message_data: serde_json::Value,
        primary_type: String,
        signature: alloy::primitives::Signature,
    ) {
        let payload = WebhookSigningPayload::typed_data_signed(
            *relayer_id,
            chain_id,
            domain_data,
            message_data,
            primary_type,
            signature,
        );
        self.queue_signing_webhook(relayer_id, chain_id, payload).await;
    }

    /// Get webhook configurations that should receive low balance alerts for a specific chain
    pub fn get_webhook_configs_for_chain(&self, chain_id: &ChainId) -> Vec<&WebhookConfig> {
        self.webhook_configs
            .iter()
            .filter(|config| {
                config.alert_on_low_balances.unwrap_or(false)
                    && (config.networks.is_empty()
                        || config.networks.contains(&chain_id.to_string()))
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn queue_low_balance_webhook(
        &self,
        relayer_id: &str,
        address: &crate::common_types::EvmAddress,
        chain_id: ChainId,
        current_balance: U256,
        minimum_balance: U256,
        current_balance_formatted: String,
        minimum_balance_formatted: String,
    ) {
        use crate::webhooks::WebhookLowBalancePayload;

        let payload = WebhookLowBalancePayload::new(
            relayer_id.to_string(),
            *address,
            chain_id,
            current_balance,
            minimum_balance,
            current_balance_formatted,
            minimum_balance_formatted,
        );

        if let Ok(payload_json) = payload.to_json_value() {
            self.queue_low_balance_webhook_internal(chain_id, payload.event_type, payload_json)
                .await;
        } else {
            error!("Failed to serialize low balance webhook payload");
        }
    }

    async fn queue_low_balance_webhook_internal(
        &self,
        chain_id: ChainId,
        event_type: crate::webhooks::types::WebhookEventType,
        payload_json: serde_json::Value,
    ) {
        if self.webhook_configs.is_empty() {
            info!("No webhooks configured, skipping low balance webhook for chain {}", chain_id);
            return;
        }

        let network_names = self.network_names.read().await;
        let chain_name =
            network_names.get(&chain_id).cloned().unwrap_or_else(|| chain_id.to_string());

        let mut deliveries_to_queue = Vec::new();

        for webhook_config in &self.webhook_configs {
            if webhook_config.alert_on_low_balances.unwrap_or(false)
                && (webhook_config.networks.is_empty()
                    || webhook_config.networks.contains(&chain_name))
            {
                use crate::webhooks::types::WebhookDelivery;
                let delivery = WebhookDelivery::new(
                    webhook_config.clone(),
                    event_type.clone(),
                    payload_json.clone(),
                );
                deliveries_to_queue.push(delivery);
            }
        }

        if deliveries_to_queue.is_empty() {
            debug!("No webhooks configured for low balance alerts on chain {}", chain_name);
            return;
        }

        info!(
            "Queuing {} low balance webhooks for chain {}",
            deliveries_to_queue.len(),
            chain_name
        );

        let mut pending = self.pending_deliveries.write().await;
        for delivery in deliveries_to_queue {
            pending.insert(delivery.id, delivery);
        }

        tokio::spawn({
            let manager = self.clone();
            async move {
                manager.process_ready_deliveries().await;
            }
        });
    }
}
