use super::types::TransactionSpeed;
use crate::rate_limiting::RateLimiter;
use crate::shared::utils::convert_blob_strings_to_blobs;
use crate::shared::{internal_server_error, HttpError};
use crate::{
    app_state::AppState,
    rate_limiting::RateLimitOperation,
    relayer::RelayerId,
    shared::common_types::EvmAddress,
    transaction::{
        queue_system::TransactionToSend,
        types::{TransactionData, TransactionHash, TransactionId, TransactionValue},
    },
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayTransactionRequest {
    pub to: EvmAddress,
    #[serde(default)]
    pub value: TransactionValue,
    #[serde(default)]
    pub data: TransactionData,
    pub speed: Option<TransactionSpeed>,
    /// This allows an app to pass their own custom external id in perfect for webhooks
    pub external_id: Option<String>,
    #[serde(default)]
    pub blobs: Option<Vec<String>>, // will overflow the stack if you use the Blob type directly
}

impl FromStr for RelayTransactionRequest {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub id: TransactionId,
    pub hash: TransactionHash,
}

/// API endpoint to send a new transaction through a relayer.
pub async fn send_transaction(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(transaction): Json<RelayTransactionRequest>,
) -> Result<Json<SendTransactionResult>, HttpError> {
    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        &state,
        &headers,
        &relayer_id,
        RateLimitOperation::Transaction,
    )
    .await?;

    let transaction_to_send = TransactionToSend::new(
        transaction.to,
        transaction.value,
        transaction.data.clone(),
        transaction.speed.clone(),
        convert_blob_strings_to_blobs(transaction.blobs)?,
        transaction.external_id,
    );

    let transaction = state
        .transactions_queues
        .lock()
        .await
        .add_transaction(&relayer_id, &transaction_to_send)
        .await?;

    let result = SendTransactionResult {
        id: transaction.id,
        hash: transaction.known_transaction_hash.ok_or(internal_server_error(Some(
            "should always have a known transaction hash".to_string(),
        )))?,
    };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
