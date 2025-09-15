use crate::{
    network::types::ChainId,
    relayer::types::RelayerId,
    shared::common_types::EvmAddress,
    transaction::types::{
        Transaction, TransactionData, TransactionHash, TransactionId, TransactionStatus,
        TransactionValue,
    },
};
use alloy::network::AnyTransactionReceipt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::WebhookEventType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Event type that triggered the webhook
    pub event_type: WebhookEventType,
    /// Transaction information
    pub transaction: WebhookTransactionData,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// API version for payload compatibility
    pub api_version: String,
    /// Original transaction data (for replacement events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_transaction: Option<WebhookTransactionData>,
    /// Transaction receipt (for mined/confirmed events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<AnyTransactionReceipt>,
}

/// Transaction data optimized for webhook payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookTransactionData {
    /// Transaction ID
    pub id: TransactionId,
    /// Relayer ID that processed this transaction
    #[serde(rename = "relayerId")]
    pub relayer_id: RelayerId,
    /// Transaction recipient address
    pub to: EvmAddress,
    /// Transaction sender address (relayer address)
    pub from: EvmAddress,
    /// Transaction value in wei
    pub value: TransactionValue,
    /// Transaction data/input
    pub data: TransactionData,
    /// Chain ID where transaction was sent
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
    /// Current transaction status
    pub status: TransactionStatus,
    /// Transaction hash (available after sending)
    #[serde(rename = "txHash", skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<TransactionHash>,
    /// When transaction was queued
    #[serde(rename = "queuedAt")]
    pub queued_at: DateTime<Utc>,
    /// When transaction was sent (if applicable)
    #[serde(rename = "sentAt", skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<DateTime<Utc>>,
    /// When transaction was confirmed (if applicable)
    #[serde(rename = "confirmedAt", skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<DateTime<Utc>>,
    /// Transaction expiration time
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
}

impl From<&Transaction> for WebhookTransactionData {
    fn from(transaction: &Transaction) -> Self {
        Self {
            id: transaction.id.clone(),
            relayer_id: transaction.relayer_id.clone(),
            to: transaction.to,
            from: transaction.from,
            value: transaction.value.clone(),
            data: transaction.data.clone(),
            chain_id: transaction.chain_id,
            status: transaction.status,
            transaction_hash: transaction.known_transaction_hash.clone(),
            queued_at: transaction.queued_at.into(),
            sent_at: transaction.sent_at.map(|dt| dt.into()),
            confirmed_at: transaction.confirmed_at.map(|dt| dt.into()),
            expires_at: transaction.expires_at.into(),
        }
    }
}

impl WebhookPayload {
    /// Create a new webhook payload from transaction and event type
    pub fn new(transaction: &Transaction, event_type: WebhookEventType) -> Self {
        Self {
            event_type,
            transaction: WebhookTransactionData::from(transaction),
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
            original_transaction: None,
            receipt: None,
        }
    }

    /// Create a new webhook payload for replacement events with original transaction
    pub fn new_with_original(
        transaction: &Transaction,
        event_type: WebhookEventType,
        original_transaction: &Transaction,
    ) -> Self {
        Self {
            event_type,
            transaction: WebhookTransactionData::from(transaction),
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
            original_transaction: Some(WebhookTransactionData::from(original_transaction)),
            receipt: None,
        }
    }

    /// Create a new webhook payload with transaction receipt
    pub fn new_with_receipt(
        transaction: &Transaction,
        event_type: WebhookEventType,
        receipt: &AnyTransactionReceipt,
    ) -> Self {
        Self {
            event_type,
            transaction: WebhookTransactionData::from(transaction),
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
            original_transaction: None,
            receipt: Some(receipt.clone()),
        }
    }

    /// Create payload for transaction queued event
    pub fn transaction_queued(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionQueued)
    }

    /// Create payload for transaction sent event
    pub fn transaction_sent(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionSent)
    }

    /// Create payload for transaction mined event
    pub fn transaction_mined(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionMined)
    }

    /// Create payload for transaction mined event with receipt
    pub fn transaction_mined_with_receipt(
        transaction: &Transaction,
        receipt: &AnyTransactionReceipt,
    ) -> Self {
        Self::new_with_receipt(transaction, WebhookEventType::TransactionMined, receipt)
    }

    /// Create payload for transaction confirmed event
    pub fn transaction_confirmed(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionConfirmed)
    }

    /// Create payload for transaction confirmed event with receipt
    pub fn transaction_confirmed_with_receipt(
        transaction: &Transaction,
        receipt: &AnyTransactionReceipt,
    ) -> Self {
        Self::new_with_receipt(transaction, WebhookEventType::TransactionConfirmed, receipt)
    }

    /// Create payload for transaction failed event
    pub fn transaction_failed(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionFailed)
    }

    /// Create payload for transaction expired event
    pub fn transaction_expired(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionExpired)
    }

    /// Create payload for transaction cancelled event
    pub fn transaction_cancelled(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionCancelled)
    }

    /// Create payload for transaction replaced event
    pub fn transaction_replaced(
        new_transaction: &Transaction,
        original_transaction: &Transaction,
    ) -> Self {
        Self::new_with_original(
            new_transaction,
            WebhookEventType::TransactionReplaced,
            original_transaction,
        )
    }

    /// Convert the payload to JSON value
    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}
