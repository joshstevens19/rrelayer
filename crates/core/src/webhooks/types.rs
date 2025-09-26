use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

use crate::{
    transaction::types::{Transaction, TransactionStatus},
    yaml::WebhookConfig,
};

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
    /// Text message was signed
    TextSigned,
    /// Typed data (EIP-712) was signed
    TypedDataSigned,
}

impl From<TransactionStatus> for WebhookEventType {
    fn from(status: TransactionStatus) -> Self {
        match status {
            TransactionStatus::PENDING => WebhookEventType::TransactionQueued,
            TransactionStatus::INMEMPOOL => WebhookEventType::TransactionSent,
            TransactionStatus::MINED => WebhookEventType::TransactionMined,
            TransactionStatus::CONFIRMED => WebhookEventType::TransactionConfirmed,
            TransactionStatus::FAILED => WebhookEventType::TransactionFailed,
            TransactionStatus::EXPIRED => WebhookEventType::TransactionExpired,
        }
    }
}

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
            initial_retry_delay_ms: 1000,
            max_retry_delay_ms: 120000,
            retry_multiplier: 2.0,
        }
    }
}

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
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            max_retries: webhook_config.max_retries.unwrap_or(3),
            webhook_config,
            event_type,
            payload,
            attempts: 0,
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
            None => true,
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

pub struct WebhookFilter;

impl WebhookFilter {
    pub fn should_send_webhook(
        webhook_config: &WebhookConfig,
        _transaction: &Transaction,
        chain_name: &str,
    ) -> bool {
        webhook_config.networks.is_empty()
            || webhook_config.networks.contains(&chain_name.to_string())
            || webhook_config.networks.contains(&"*".to_string())
    }
}
