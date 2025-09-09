use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

use crate::{
    transaction::types::{Transaction, TransactionStatus},
    yaml::WebhookConfig,
};

/// Webhook event types that trigger webhook calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Transaction was added to the queue
    TransactionQueued,
    /// Transaction was sent to the blockchain
    TransactionSent,
    /// Transaction was mined (included in a block)
    TransactionMined,
    /// Transaction was confirmed (reached required confirmations)
    TransactionConfirmed,
    /// Transaction failed
    TransactionFailed,
    /// Transaction expired
    TransactionExpired,
    /// Transaction was cancelled
    TransactionCancelled,
    /// Transaction was replaced
    TransactionReplaced,
}

impl From<TransactionStatus> for WebhookEventType {
    fn from(status: TransactionStatus) -> Self {
        match status {
            TransactionStatus::Pending => WebhookEventType::TransactionQueued,
            TransactionStatus::Inmempool => WebhookEventType::TransactionSent,
            TransactionStatus::Mined => WebhookEventType::TransactionMined,
            TransactionStatus::Confirmed => WebhookEventType::TransactionConfirmed,
            TransactionStatus::Failed => WebhookEventType::TransactionFailed,
            TransactionStatus::Expired => WebhookEventType::TransactionExpired,
        }
    }
}

/// Configuration for webhook delivery with defaults
#[derive(Debug, Clone)]
pub struct WebhookDeliveryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Timeout for each webhook request in seconds
    pub timeout_seconds: u32,
    /// Initial retry delay in milliseconds
    pub initial_retry_delay_ms: u64,
    /// Maximum retry delay in milliseconds
    pub max_retry_delay_ms: u64,
    /// Exponential backoff multiplier
    pub retry_multiplier: f32,
}

impl Default for WebhookDeliveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_seconds: 30,
            initial_retry_delay_ms: 1000, // 1 second
            max_retry_delay_ms: 60000,    // 1 minute
            retry_multiplier: 2.0,
        }
    }
}

/// Webhook delivery attempt tracking
#[derive(Debug, Clone)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_config: WebhookConfig,
    pub event_type: WebhookEventType,
    pub payload: serde_json::Value,
    pub attempts: u32,
    pub max_retries: u32,
    pub created_at: SystemTime,
    pub last_attempt_at: Option<SystemTime>,
    pub next_retry_at: Option<SystemTime>,
    pub completed: bool,
    pub failed: bool,
    pub error_message: Option<String>,
}

impl WebhookDelivery {
    pub fn new(
        webhook_config: WebhookConfig,
        event_type: WebhookEventType,
        payload: serde_json::Value,
        max_retries: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            webhook_config,
            event_type,
            payload,
            attempts: 0,
            max_retries,
            created_at: SystemTime::now(),
            last_attempt_at: None,
            next_retry_at: None,
            completed: false,
            failed: false,
            error_message: None,
        }
    }

    pub fn should_retry(&self) -> bool {
        !self.completed && !self.failed && self.attempts < self.max_retries
    }

    pub fn is_ready_for_retry(&self, now: SystemTime) -> bool {
        if !self.should_retry() {
            return false;
        }

        match self.next_retry_at {
            Some(retry_time) => now >= retry_time,
            None => true, // First attempt
        }
    }

    pub fn mark_attempt(&mut self, now: SystemTime, next_retry_delay_ms: Option<u64>) {
        self.attempts += 1;
        self.last_attempt_at = Some(now);

        if let Some(delay_ms) = next_retry_delay_ms {
            self.next_retry_at = Some(now + std::time::Duration::from_millis(delay_ms));
        }
    }

    pub fn mark_completed(&mut self) {
        self.completed = true;
        self.next_retry_at = None;
    }

    pub fn mark_failed(&mut self, error: String) {
        self.failed = true;
        self.error_message = Some(error);
        self.next_retry_at = None;
    }
}

/// Filter for determining which webhooks should receive which events
pub struct WebhookFilter;

impl WebhookFilter {
    /// Check if a webhook should receive an event for a specific transaction
    pub fn should_send_webhook(
        webhook_config: &WebhookConfig,
        _transaction: &Transaction,
        chain_name: &str,
    ) -> bool {
        // Check if the webhook is configured for this network
        webhook_config.networks.is_empty()
            || webhook_config.networks.contains(&chain_name.to_string())
            || webhook_config.networks.contains(&"*".to_string())
    }
}
