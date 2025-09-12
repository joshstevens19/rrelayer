use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
};

/// Pauses transaction processing for a relayer.
///
/// This endpoint stops the relayer from processing new transactions while keeping
/// it in the system. The relayer's queue is paused and the status is updated in the database.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer to pause
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If pause succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn pause_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.pause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_paused(true);
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
