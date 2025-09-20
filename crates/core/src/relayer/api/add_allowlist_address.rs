use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::relayer::cache::invalidate_relayer_cache;
use crate::{
    app_state::AppState,
    relayer::{get_relayer, types::RelayerId},
    shared::common_types::EvmAddress,
};

/// Adds an address to the relayer's allowlist.
///
/// This endpoint adds an Ethereum address to the relayer's allowlist and automatically
/// enables allowlist-only mode for the relayer's transaction queue.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `address` - The Ethereum address to add to the allowlist
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If addition succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn add_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> StatusCode {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.relayer_add_allowlist_address(&relayer_id, &address).await {
            Ok(_) => {
                invalidate_relayer_cache(&state.cache, &relayer_id).await;
                if let Ok(queue) = state
                    .transactions_queues
                    .lock()
                    .await
                    .get_transactions_queue_unsafe(&relayer_id)
                {
                    queue.lock().await.set_is_allowlisted_only(true);
                }

                StatusCode::NO_CONTENT
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
