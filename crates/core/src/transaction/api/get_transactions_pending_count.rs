use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{app_state::AppState, relayer::types::RelayerId};

/// API endpoint to get the count of pending transactions for a relayer.
///
/// Returns the number of transactions currently pending in the queue for the given relayer.
///
/// # Arguments
/// * `state` - The application state containing transaction queues
/// * `relayer_id` - The relayer ID path parameter
///
/// # Returns
/// * `Ok(Json<usize>)` - The count of pending transactions
pub async fn get_transactions_pending_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<usize>, StatusCode> {
    let count =
        state.transactions_queues.lock().await.pending_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
