use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::relayer::get_relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
};

/// Resumes transaction processing for a paused relayer.
pub async fn unpause_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<StatusCode, HttpError> {
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        state.db.unpause_relayer(&relayer_id).await?;
        invalidate_relayer_cache(&state.cache, &relayer_id).await;
        if let Ok(queue) =
            state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
        {
            queue.lock().await.set_is_paused(false);
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
