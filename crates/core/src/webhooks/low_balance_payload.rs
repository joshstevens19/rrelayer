use crate::{network::ChainId, shared::common_types::EvmAddress};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::WebhookEventType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookLowBalancePayload {
    /// Event type that triggered the webhook
    pub event_type: WebhookEventType,
    /// Low balance alert data
    pub balance_alert: WebhookBalanceAlertData,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// API version for payload compatibility
    pub api_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookBalanceAlertData {
    /// Relayer ID with low balance
    #[serde(rename = "relayerId")]
    pub relayer_id: String,
    /// Relayer address with low balance
    pub address: EvmAddress,
    /// Chain ID where low balance was detected
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
    /// Current balance in wei
    #[serde(rename = "currentBalance")]
    pub current_balance: String,
    /// Minimum recommended balance in wei
    #[serde(rename = "minimumBalance")]
    pub minimum_balance: String,
    /// Current balance formatted in ETH/native token
    #[serde(rename = "currentBalanceFormatted")]
    pub current_balance_formatted: String,
    /// Minimum balance formatted in ETH/native token
    #[serde(rename = "minimumBalanceFormatted")]
    pub minimum_balance_formatted: String,
    /// When the alert was triggered
    #[serde(rename = "detectedAt")]
    pub detected_at: DateTime<Utc>,
}

impl WebhookLowBalancePayload {
    pub fn new(
        relayer_id: String,
        address: EvmAddress,
        chain_id: ChainId,
        current_balance: u128,
        minimum_balance: u128,
        current_balance_formatted: String,
        minimum_balance_formatted: String,
    ) -> Self {
        Self {
            event_type: WebhookEventType::LowBalance,
            balance_alert: WebhookBalanceAlertData {
                relayer_id: relayer_id.clone(),
                address,
                chain_id,
                current_balance: current_balance.to_string(),
                minimum_balance: minimum_balance.to_string(),
                current_balance_formatted,
                minimum_balance_formatted,
                detected_at: Utc::now(),
            },
            timestamp: Utc::now(),
            api_version: "1.0".to_string(),
        }
    }

    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}
