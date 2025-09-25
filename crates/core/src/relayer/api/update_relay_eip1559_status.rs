use std::sync::Arc;

use crate::relayer::cache::invalidate_relayer_cache;
use crate::relayer::get_relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::types::RelayerId};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    http::StatusCode,
};

/// Updates the EIP-1559 transaction status for a relayer.
pub async fn update_relay_eip1559_status(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, enabled)): Path<(RelayerId, bool)>,
    headers: HeaderMap,
) -> Result<StatusCode, HttpError> {
    state.validate_basic_auth_valid(&headers)?;
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        state.db.update_relayer_eip_1559_status(&relayer_id, &enabled).await?;
        invalidate_relayer_cache(&state.cache, &relayer_id).await;
        if let Ok(queue) =
            state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
        {
            queue.lock().await.set_is_legacy_transactions(!enabled); // Fixed: EIP-1559 enabled = NOT legacy
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
