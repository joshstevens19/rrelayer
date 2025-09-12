use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
};

/// Resumes transaction processing for a paused relayer.
///
/// This endpoint reactivates a paused relayer, allowing it to resume processing
/// transactions. The relayer's queue is unpaused and the status is updated in the database.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer to unpause
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If unpause succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn unpause_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.unpause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_paused(false);
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
