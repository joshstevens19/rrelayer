use super::types::TransactionSpeed;
use crate::rate_limiting::RateLimiter;
use crate::shared::utils::convert_blob_strings_to_blobs;
use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    rate_limiting::{RateLimitError, RateLimitOperation},
    relayer::{get_relayer, RelayerId},
    rrelayer_error,
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
use tracing::error;

/// Request structure for relaying transactions.
///
/// Contains all necessary information to create and relay a blockchain transaction.
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

    /// Parses a RelayTransactionRequest from a JSON string.
    ///
    /// # Arguments
    /// * `s` - The JSON string to parse
    ///
    /// # Returns
    /// * `Ok(RelayTransactionRequest)` - The parsed request
    /// * `Err(serde_json::Error)` - If JSON parsing fails
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

/// Response structure for send transaction requests.
///
/// Contains the assigned transaction ID and the blockchain transaction hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub id: TransactionId,
    pub hash: TransactionHash,
}

/// API endpoint to send a new transaction through a relayer.
///
/// Creates a new transaction and adds it to the transaction queue for processing.
///
/// # Arguments
/// * `state` - The application state containing transaction queues and other services
/// * `relayer_id` - The relayer ID path parameter
/// * `headers` - HTTP headers (for future API key validation)
/// * `transaction` - The transaction request payload
///
/// # Returns
/// * `Ok(Json<SendTransactionResult>)` - The transaction ID and hash if successful
/// * `Err(StatusCode)` - BAD_REQUEST for invalid transactions, INTERNAL_SERVER_ERROR for other failures
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
        .await
        .map_err(|e| {
            error!("{}", e);
            StatusCode::BAD_REQUEST
        })?;

    let result = SendTransactionResult {
        id: transaction.id,
        hash: transaction.known_transaction_hash.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
