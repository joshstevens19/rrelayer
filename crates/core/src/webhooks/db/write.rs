use bytes::BytesMut;
use chrono::{DateTime, Utc};
use core::fmt;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};
use tokio_postgres::types::{FromSql, IsNull, Type};
use uuid::Uuid;

use crate::{
    network::ChainId,
    postgres::{PostgresClient, PostgresError, ToSql},
    relayer::RelayerId,
    transaction::types::TransactionId,
    webhooks::types::WebhookEventType,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum WebhookDeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Abandoned,
}

impl Display for WebhookDeliveryStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl WebhookDeliveryStatus {
    pub fn format(&self) -> String {
        match self {
            WebhookDeliveryStatus::Pending => "PENDING".to_string(),
            WebhookDeliveryStatus::Delivered => "DELIVERED".to_string(),
            WebhookDeliveryStatus::Failed => "FAILED".to_string(),
            WebhookDeliveryStatus::Abandoned => "ABANDONED".to_string(),
        }
    }
}

impl<'a> FromSql<'a> for WebhookDeliveryStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "status" {
            let status =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match status {
                "PENDING" => Ok(WebhookDeliveryStatus::Pending),
                "DELIVERED" => Ok(WebhookDeliveryStatus::Delivered),
                "FAILED" => Ok(WebhookDeliveryStatus::Failed),
                "ABANDONED" => Ok(WebhookDeliveryStatus::Abandoned),
                _ => Err(format!("Unknown WebhookDeliveryStatus: {}", status).into()),
            }
        } else if *ty == Type::TEXT
            || *ty == Type::CHAR
            || *ty == Type::VARCHAR
            || *ty == Type::BPCHAR
        {
            let status =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match status {
                "PENDING" => Ok(WebhookDeliveryStatus::Pending),
                "DELIVERED" => Ok(WebhookDeliveryStatus::Delivered),
                "FAILED" => Ok(WebhookDeliveryStatus::Failed),
                "ABANDONED" => Ok(WebhookDeliveryStatus::Abandoned),
                _ => Err(format!("Unknown WebhookDeliveryStatus: {}", status).into()),
            }
        } else {
            Err(format!("Unexpected type for WebhookDeliveryStatus: {}", ty).into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "status")
    }
}

impl ToSql for WebhookDeliveryStatus {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for WebhookDeliveryStatus: {}", ty).into());
        }

        let status_str = match self {
            WebhookDeliveryStatus::Pending => "PENDING",
            WebhookDeliveryStatus::Delivered => "DELIVERED",
            WebhookDeliveryStatus::Failed => "FAILED",
            WebhookDeliveryStatus::Abandoned => "ABANDONED",
        };

        out.extend_from_slice(status_str.as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "status")
    }

    tokio_postgres::types::to_sql_checked!();
}

/// Database event type mapping for webhook deliveries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum WebhookDeliveryEventType {
    TransactionQueued,
    TransactionSent,
    TransactionMined,
    TransactionConfirmed,
    TransactionFailed,
    TransactionExpired,
    TransactionCancelled,
    TransactionReplaced,
    TextSigned,
    TypedDataSigned,
    LowBalance,
}

impl Display for WebhookDeliveryEventType {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl WebhookDeliveryEventType {
    pub fn format(&self) -> String {
        match self {
            WebhookDeliveryEventType::TransactionQueued => "TRANSACTION_QUEUED".to_string(),
            WebhookDeliveryEventType::TransactionSent => "TRANSACTION_SENT".to_string(),
            WebhookDeliveryEventType::TransactionMined => "TRANSACTION_MINED".to_string(),
            WebhookDeliveryEventType::TransactionConfirmed => "TRANSACTION_CONFIRMED".to_string(),
            WebhookDeliveryEventType::TransactionFailed => "TRANSACTION_FAILED".to_string(),
            WebhookDeliveryEventType::TransactionExpired => "TRANSACTION_EXPIRED".to_string(),
            WebhookDeliveryEventType::TransactionCancelled => "TRANSACTION_CANCELLED".to_string(),
            WebhookDeliveryEventType::TransactionReplaced => "TRANSACTION_REPLACED".to_string(),
            WebhookDeliveryEventType::TextSigned => "TEXT_SIGNED".to_string(),
            WebhookDeliveryEventType::TypedDataSigned => "TYPED_DATA_SIGNED".to_string(),
            WebhookDeliveryEventType::LowBalance => "LOW_BALANCE".to_string(),
        }
    }
}

impl From<WebhookEventType> for WebhookDeliveryEventType {
    fn from(event_type: WebhookEventType) -> Self {
        match event_type {
            WebhookEventType::TransactionQueued => WebhookDeliveryEventType::TransactionQueued,
            WebhookEventType::TransactionSent => WebhookDeliveryEventType::TransactionSent,
            WebhookEventType::TransactionMined => WebhookDeliveryEventType::TransactionMined,
            WebhookEventType::TransactionConfirmed => {
                WebhookDeliveryEventType::TransactionConfirmed
            }
            WebhookEventType::TransactionFailed => WebhookDeliveryEventType::TransactionFailed,
            WebhookEventType::TransactionExpired => WebhookDeliveryEventType::TransactionExpired,
            WebhookEventType::TransactionCancelled => {
                WebhookDeliveryEventType::TransactionCancelled
            }
            WebhookEventType::TransactionReplaced => WebhookDeliveryEventType::TransactionReplaced,
            WebhookEventType::TextSigned => WebhookDeliveryEventType::TextSigned,
            WebhookEventType::TypedDataSigned => WebhookDeliveryEventType::TypedDataSigned,
            WebhookEventType::LowBalance => WebhookDeliveryEventType::LowBalance,
        }
    }
}

impl<'a> FromSql<'a> for WebhookDeliveryEventType {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "event_type" {
            let event_type =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match event_type {
                "TRANSACTION_QUEUED" => Ok(WebhookDeliveryEventType::TransactionQueued),
                "TRANSACTION_SENT" => Ok(WebhookDeliveryEventType::TransactionSent),
                "TRANSACTION_MINED" => Ok(WebhookDeliveryEventType::TransactionMined),
                "TRANSACTION_CONFIRMED" => Ok(WebhookDeliveryEventType::TransactionConfirmed),
                "TRANSACTION_FAILED" => Ok(WebhookDeliveryEventType::TransactionFailed),
                "TRANSACTION_EXPIRED" => Ok(WebhookDeliveryEventType::TransactionExpired),
                "TRANSACTION_CANCELLED" => Ok(WebhookDeliveryEventType::TransactionCancelled),
                "TRANSACTION_REPLACED" => Ok(WebhookDeliveryEventType::TransactionReplaced),
                "TEXT_SIGNED" => Ok(WebhookDeliveryEventType::TextSigned),
                "TYPED_DATA_SIGNED" => Ok(WebhookDeliveryEventType::TypedDataSigned),
                "LOW_BALANCE" => Ok(WebhookDeliveryEventType::LowBalance),
                _ => Err(format!("Unknown WebhookDeliveryEventType: {}", event_type).into()),
            }
        } else if *ty == Type::TEXT
            || *ty == Type::CHAR
            || *ty == Type::VARCHAR
            || *ty == Type::BPCHAR
        {
            let event_type =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match event_type {
                "TRANSACTION_QUEUED" => Ok(WebhookDeliveryEventType::TransactionQueued),
                "TRANSACTION_SENT" => Ok(WebhookDeliveryEventType::TransactionSent),
                "TRANSACTION_MINED" => Ok(WebhookDeliveryEventType::TransactionMined),
                "TRANSACTION_CONFIRMED" => Ok(WebhookDeliveryEventType::TransactionConfirmed),
                "TRANSACTION_FAILED" => Ok(WebhookDeliveryEventType::TransactionFailed),
                "TRANSACTION_EXPIRED" => Ok(WebhookDeliveryEventType::TransactionExpired),
                "TRANSACTION_CANCELLED" => Ok(WebhookDeliveryEventType::TransactionCancelled),
                "TRANSACTION_REPLACED" => Ok(WebhookDeliveryEventType::TransactionReplaced),
                "TEXT_SIGNED" => Ok(WebhookDeliveryEventType::TextSigned),
                "TYPED_DATA_SIGNED" => Ok(WebhookDeliveryEventType::TypedDataSigned),
                "LOW_BALANCE" => Ok(WebhookDeliveryEventType::LowBalance),
                _ => Err(format!("Unknown WebhookDeliveryEventType: {}", event_type).into()),
            }
        } else {
            Err(format!("Unexpected type for WebhookDeliveryEventType: {}", ty).into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "event_type")
    }
}

impl ToSql for WebhookDeliveryEventType {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for WebhookDeliveryEventType: {}", ty).into());
        }

        let event_type_str = match self {
            WebhookDeliveryEventType::TransactionQueued => "TRANSACTION_QUEUED",
            WebhookDeliveryEventType::TransactionSent => "TRANSACTION_SENT",
            WebhookDeliveryEventType::TransactionMined => "TRANSACTION_MINED",
            WebhookDeliveryEventType::TransactionConfirmed => "TRANSACTION_CONFIRMED",
            WebhookDeliveryEventType::TransactionFailed => "TRANSACTION_FAILED",
            WebhookDeliveryEventType::TransactionExpired => "TRANSACTION_EXPIRED",
            WebhookDeliveryEventType::TransactionCancelled => "TRANSACTION_CANCELLED",
            WebhookDeliveryEventType::TransactionReplaced => "TRANSACTION_REPLACED",
            WebhookDeliveryEventType::TextSigned => "TEXT_SIGNED",
            WebhookDeliveryEventType::TypedDataSigned => "TYPED_DATA_SIGNED",
            WebhookDeliveryEventType::LowBalance => "LOW_BALANCE",
        };

        out.extend_from_slice(event_type_str.as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "event_type")
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWebhookDeliveryRequest {
    pub id: Uuid,
    pub webhook_endpoint: String,
    pub event_type: WebhookEventType,
    pub status: WebhookDeliveryStatus,
    pub transaction_id: Option<TransactionId>,
    pub relayer_id: Option<RelayerId>,
    pub chain_id: Option<ChainId>,
    pub attempts: i32,
    pub max_retries: i32,
    pub payload: serde_json::Value,
    pub headers: Option<serde_json::Value>,
    pub first_attempt_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWebhookDeliveryRequest {
    pub id: Uuid,
    pub status: WebhookDeliveryStatus,
    pub attempts: i32,
    pub http_status_code: Option<i32>,
    pub response_body: Option<String>,
    pub error_message: Option<String>,
    pub last_attempt_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub abandoned_at: Option<DateTime<Utc>>,
    pub total_duration_ms: Option<i64>,
}

impl PostgresClient {
    pub async fn create_webhook_delivery(
        &self,
        request: &CreateWebhookDeliveryRequest,
    ) -> Result<(), PostgresError> {
        let query = r#"
            INSERT INTO webhook.delivery_history (
                id, webhook_endpoint, event_type, status, transaction_id, relayer_id, chain_id,
                attempts, max_retries, payload, headers, first_attempt_at, last_attempt_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
            ON CONFLICT (id) DO NOTHING;
        "#;

        let conn = self.pool.get().await?;
        let event_type_db = WebhookDeliveryEventType::from(request.event_type.clone());

        conn.execute(
            query,
            &[
                &request.id,
                &request.webhook_endpoint,
                &event_type_db,
                &request.status,
                &request.transaction_id,
                &request.relayer_id,
                &request.chain_id,
                &request.attempts,
                &request.max_retries,
                &request.payload,
                &request.headers,
                &request.first_attempt_at,
            ],
        )
        .await?;

        Ok(())
    }

    pub async fn update_webhook_delivery(
        &self,
        request: &UpdateWebhookDeliveryRequest,
    ) -> Result<(), PostgresError> {
        let query = r#"
            UPDATE webhook.delivery_history 
            SET 
                status = $2,
                attempts = $3,
                http_status_code = $4,
                response_body = $5,
                error_message = $6,
                last_attempt_at = $7,
                delivered_at = $8,
                abandoned_at = $9,
                total_duration_ms = $10
            WHERE id = $1;
        "#;

        let conn = self.pool.get().await?;

        conn.execute(
            query,
            &[
                &request.id,
                &request.status,
                &request.attempts,
                &request.http_status_code,
                &request.response_body,
                &request.error_message,
                &request.last_attempt_at,
                &request.delivered_at,
                &request.abandoned_at,
                &request.total_duration_ms,
            ],
        )
        .await?;

        Ok(())
    }

    pub async fn cleanup_old_webhook_deliveries(&self) -> Result<u64, PostgresError> {
        let query = "SELECT cleanup_old_webhook_deliveries();";
        let conn = self.pool.get().await?;
        let rows_affected = conn.execute(query, &[]).await?;
        Ok(rows_affected)
    }
}
