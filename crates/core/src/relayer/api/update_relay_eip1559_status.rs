use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{app_state::AppState, relayer::types::RelayerId};
use crate::relayer::cache::invalidate_relayer_cache;

/// Updates the EIP-1559 transaction status for a relayer.
///
/// This endpoint enables or disables EIP-1559 (London hard fork) transaction support
/// for a relayer. When enabled, the relayer will use type-2 transactions with base fee
/// and priority fee. When disabled, it uses legacy transactions with gas price.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `enabled` - Whether to enable EIP-1559 transactions (true) or use legacy (false)
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If update succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn update_relay_eip1559_status(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, enabled)): Path<(RelayerId, bool)>,
) -> StatusCode {
    match state.db.update_relayer_eip_1559_status(&relayer_id, &enabled).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_legacy_transactions(!enabled); // Fixed: EIP-1559 enabled = NOT legacy
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
