use crate::{
    gas::{MaxFee, MaxPriorityFee},
    network::ChainId,
    relayer::RelayerId,
    shared::common_types::{BlockNumber, EvmAddress},
    transaction::types::{
        Transaction, TransactionBlob, TransactionData, TransactionHash, TransactionId,
        TransactionNonce, TransactionStatus, TransactionValue,
    },
};
use alloy::{network::AnyTransactionReceipt, primitives::PrimitiveSignature};
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
    /// External ID for transaction tracking
    #[serde(rename = "externalId", skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Transaction blobs (for blob transactions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blobs: Option<Vec<TransactionBlob>>,
    /// Transaction nonce
    pub nonce: TransactionNonce,
    /// When transaction was mined (if applicable)
    #[serde(rename = "minedAt", skip_serializing_if = "Option::is_none", default)]
    pub mined_at: Option<DateTime<Utc>>,
    /// Block number when transaction was mined
    #[serde(rename = "minedAtBlockNumber", skip_serializing_if = "Option::is_none", default)]
    pub mined_at_block_number: Option<BlockNumber>,
    /// Max priority fee per gas used when sending
    #[serde(rename = "maxPriorityFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_priority_fee_per_gas: Option<MaxPriorityFee>,
    /// Max fee per gas used when sending
    #[serde(rename = "maxFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_fee_per_gas: Option<MaxFee>,
    /// Whether this is a no-op transaction
    pub is_noop: bool,
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
            external_id: transaction.external_id.clone(),
            blobs: transaction.blobs.clone(),
            nonce: transaction.nonce.clone(),
            mined_at: transaction.mined_at.map(|dt| dt.into()),
            mined_at_block_number: transaction.mined_at_block_number,
            sent_with_max_priority_fee_per_gas: transaction
                .sent_with_max_priority_fee_per_gas
                .clone(),
            sent_with_max_fee_per_gas: transaction.sent_with_max_fee_per_gas.clone(),
            is_noop: transaction.is_noop,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSigningPayload {
    /// Event type that triggered the webhook
    pub event_type: WebhookEventType,
    /// Signing operation data
    pub signing: WebhookSigningData,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// API version for payload compatibility
    pub api_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSigningData {
    /// Relayer ID that performed the signing
    #[serde(rename = "relayerId")]
    pub relayer_id: RelayerId,
    /// Chain ID where signing occurred
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
    /// The signature produced
    pub signature: PrimitiveSignature,
    /// When the signing occurred
    #[serde(rename = "signedAt")]
    pub signed_at: DateTime<Utc>,
    /// Text message (for text signing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Typed data domain (for typed data signing)
    #[serde(rename = "domainData", skip_serializing_if = "Option::is_none")]
    pub domain_data: Option<serde_json::Value>,
    /// Typed data message (for typed data signing)
    #[serde(rename = "messageData", skip_serializing_if = "Option::is_none")]
    pub message_data: Option<serde_json::Value>,
    /// Primary type (for typed data signing)
    #[serde(rename = "primaryType", skip_serializing_if = "Option::is_none")]
    pub primary_type: Option<String>,
}

impl WebhookSigningPayload {
    pub fn text_signed(
        relayer_id: RelayerId,
        chain_id: ChainId,
        message: String,
        signature: PrimitiveSignature,
    ) -> Self {
        Self {
            event_type: WebhookEventType::TextSigned,
            signing: WebhookSigningData {
                relayer_id,
                chain_id,
                signature,
                signed_at: Utc::now(),
                message: Some(message),
                domain_data: None,
                message_data: None,
                primary_type: None,
            },
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
        }
    }

    pub fn typed_data_signed(
        relayer_id: RelayerId,
        chain_id: ChainId,
        domain_data: serde_json::Value,
        message_data: serde_json::Value,
        primary_type: String,
        signature: PrimitiveSignature,
    ) -> Self {
        Self {
            event_type: WebhookEventType::TypedDataSigned,
            signing: WebhookSigningData {
                relayer_id,
                chain_id,
                signature,
                signed_at: Utc::now(),
                message: None,
                domain_data: Some(domain_data),
                message_data: Some(message_data),
                primary_type: Some(primary_type),
            },
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
        }
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl WebhookPayload {
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

    pub fn transaction_queued(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionQueued)
    }

    pub fn transaction_sent(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionSent)
    }

    pub fn transaction_mined(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionMined)
    }

    pub fn transaction_mined_with_receipt(
        transaction: &Transaction,
        receipt: &AnyTransactionReceipt,
    ) -> Self {
        Self::new_with_receipt(transaction, WebhookEventType::TransactionMined, receipt)
    }

    pub fn transaction_confirmed(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionConfirmed)
    }

    pub fn transaction_confirmed_with_receipt(
        transaction: &Transaction,
        receipt: &AnyTransactionReceipt,
    ) -> Self {
        Self::new_with_receipt(transaction, WebhookEventType::TransactionConfirmed, receipt)
    }

    pub fn transaction_failed(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionFailed)
    }

    pub fn transaction_expired(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionExpired)
    }

    pub fn transaction_cancelled(transaction: &Transaction) -> Self {
        Self::new(transaction, WebhookEventType::TransactionCancelled)
    }

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

    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}
