use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::relayer::cache::invalidate_relayer_cache;
use crate::relayer::get_relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::types::RelayerId, shared::common_types::EvmAddress};

/// Adds an address to the relayer's allowlist.
pub async fn add_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> Result<StatusCode, HttpError> {
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        state.db.relayer_add_allowlist_address(&relayer_id, &address).await?;
        invalidate_relayer_cache(&state.cache, &relayer_id).await;
        if let Ok(queue) =
            state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
        {
            queue.lock().await.set_is_allowlisted_only(true);
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
