use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::relayer::cache::invalidate_relayer_cache;
use crate::relayer::get_relayer;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::types::RelayerId, shared::common_types::EvmAddress};

/// Removes an address from the relayer's allowlist.
pub async fn delete_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> Result<StatusCode, HttpError> {
    let relayer = get_relayer(&state.db, &state.cache, &relayer_id).await?;
    if let Some(relayer) = relayer {
        state.db.relayer_delete_allowlist_address(&relayer_id, &address).await?;
        invalidate_relayer_cache(&state.cache, &relayer_id).await;
        if !relayer.allowlisted_only {
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_allowlisted_only(false);
            }
        }
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
