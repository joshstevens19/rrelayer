use super::types::TransactionSpeed;
use crate::shared::utils::convert_blob_strings_to_blobs;
use crate::{
    app_state::AppState,
    relayer::{get_relayer, types::RelayerId},
    rrelayer_error,
    shared::common_types::EvmAddress,
    transaction::{
        queue_system::TransactionToSend,
        types::{TransactionData, TransactionHash, TransactionId, TransactionValue},
    },
    user_rate_limiting::{UserDetector, UserRateLimitError},
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
) -> Result<Json<SendTransactionResult>, StatusCode> {
    if let Some(ref user_rate_limiter) = state.user_rate_limiter {
        let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let user_detection_config = state
            .rate_limit_config
            .as_ref()
            .and_then(|config| config.user_detection.clone())
            .unwrap_or_default();
        let user_detector = UserDetector::new(user_detection_config);

        let transaction_bytes = transaction.data.clone().into_inner();
        let user_context = user_detector
            .detect_user(&headers, Some(&transaction.to), &transaction_bytes, &relayer.address)
            .unwrap_or_else(|_| {
                // Fallback to relayer address if detection fails
                crate::user_rate_limiting::UserContext {
                    user_address: relayer.address,
                    detection_method: crate::user_rate_limiting::UserDetectionMethod::Fallback,
                    transaction_type: crate::user_rate_limiting::TransactionType::Direct,
                }
            });

        let user_identifier = format!("{:?}", user_context.user_address);

        match user_rate_limiter
            .check_rate_limit(&user_identifier, "transactions_per_minute", 1)
            .await
        {
            Ok(check) => {
                if !check.allowed {
                    error!("Rate limit exceeded for user {}: {}", user_identifier, check.rule_type);
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(UserRateLimitError::LimitExceeded {
                rule_type,
                current,
                limit,
                window_seconds,
            }) => {
                error!(
                    "Rate limit exceeded for user {}: {}/{} {} in {}s",
                    user_identifier, current, limit, rule_type, window_seconds
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                error!("Rate limiting error: {}", e);
                // Don't block transaction for rate limiting errors, just log
            }
        }

        tokio::spawn({
            let user_rate_limiter = user_rate_limiter.clone();
            let relayer_id = relayer_id;
            let user_context = user_context.clone();
            async move {
                let relayer_uuid: uuid::Uuid = relayer_id.into();
                let _ = user_rate_limiter
                    .record_transaction_metadata(
                        None,
                        &relayer_uuid,
                        &user_context.user_address,
                        &format!("{:?}", user_context.detection_method).to_lowercase(),
                        &format!("{:?}", user_context.transaction_type).to_lowercase(),
                        None,
                        &["transactions_per_minute".to_string()],
                    )
                    .await;
            }
        });
    }

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
            rrelayer_error!("{}", e);
            StatusCode::BAD_REQUEST
        })?;

    Ok(Json(SendTransactionResult {
        id: transaction.id,
        hash: transaction.known_transaction_hash.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    }))
}
