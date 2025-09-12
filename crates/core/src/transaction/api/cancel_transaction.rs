use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{
    app_state::AppState,
    transaction::{get_transaction_by_id, types::TransactionId},
};

/// API endpoint to cancel a pending transaction.
///
/// Cancels a pending transaction by sending a replacement with higher gas price.
///
/// # Arguments
/// * `state` - The application state containing transaction queues and database
/// * `transaction_id` - The transaction ID to cancel
///
/// # Returns
/// * `Ok(Json<bool>)` - True if cancellation was successful
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, INTERNAL_SERVER_ERROR for other failures
// TODO: should return a new tx hash
pub async fn cancel_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
) -> Result<Json<bool>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let status = state
        .transactions_queues
        .lock()
        .await
        .cancel_transaction(&transaction)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(status))
}
