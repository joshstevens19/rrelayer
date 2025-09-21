use super::send_transaction::RelayTransactionRequest;
use crate::rate_limiting::{RateLimitOperation, RateLimiter};
use crate::{
    app_state::AppState,
    transaction::{get_transaction_by_id, types::TransactionId},
};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

/// API endpoint to replace an existing pending transaction.
///
/// Replaces a pending transaction with new transaction parameters.
///
/// # Arguments
/// * `state` - The application state containing transaction queues and database
/// * `transaction_id` - The transaction ID to replace
/// * `replace_with` - The new transaction parameters
///
/// # Returns
/// * `Ok(Json<bool>)` - True if replacement was successful
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, BAD_REQUEST for invalid replacement
// TODO: should return a new tx hash
pub async fn replace_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
    Json(replace_with): Json<RelayTransactionRequest>,
) -> Result<Json<bool>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        &state,
        &headers,
        &transaction.relayer_id,
        RateLimitOperation::Transaction,
    )
    .await?;

    let status = state
        .transactions_queues
        .lock()
        .await
        .replace_transaction(&transaction, &replace_with)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(status))
}
