use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{app_state::AppState, relayer::RelayerId};

/// API endpoint to get the count of in-mempool transactions for a relayer.
///
/// Returns the number of transactions currently in the mempool for the given relayer.
///
/// # Arguments
/// * `state` - The application state containing transaction queues
/// * `relayer_id` - The relayer ID path parameter
///
/// # Returns
/// * `Ok(Json<usize>)` - The count of in-mempool transactions
pub async fn get_transactions_inmempool_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<usize>, StatusCode> {
    let count =
        state.transactions_queues.lock().await.inmempool_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
