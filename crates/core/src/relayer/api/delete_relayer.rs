use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
};

/// Soft deletes a relayer from the system.
///
/// This endpoint marks a relayer as deleted in the database, invalidates its cache,
/// and removes its transaction queue. The relayer data is preserved for audit purposes.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `relayer_id` - The unique identifier of the relayer to delete
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If deletion succeeds
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If deletion fails
pub async fn delete_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.delete_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            state.transactions_queues.lock().await.delete_queue(&relayer_id).await;
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
