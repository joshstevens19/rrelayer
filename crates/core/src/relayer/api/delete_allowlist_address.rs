use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{app_state::AppState, relayer::types::RelayerId, shared::common_types::EvmAddress};
use crate::relayer::cache::invalidate_relayer_cache;

/// Removes an address from the relayer's allowlist.
///
/// This endpoint removes an Ethereum address from the relayer's allowlist.
/// If no addresses remain in the allowlist, allowlist-only mode is automatically disabled.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `address` - The Ethereum address to remove from the allowlist
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If removal succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn delete_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> StatusCode {
    match state.db.relayer_delete_allowlist_address(&relayer_id, &address).await {
        Ok(_) => match state.db.get_relayer(&relayer_id).await { // TODO: revise this
            Ok(Some(relayer)) => {
                invalidate_relayer_cache(&state.cache, &relayer_id).await;
                if !relayer.allowlisted_only {
                    if let Ok(queue) = state
                        .transactions_queues
                        .lock()
                        .await
                        .get_transactions_queue_unsafe(&relayer_id)
                    {
                        queue.lock().await.set_is_allowlisted_only(false);
                    }
                }
                StatusCode::NO_CONTENT
            }
            Ok(None) => StatusCode::NOT_FOUND,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
