use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
};

/// Soft deletes a relayer from the system.
pub async fn delete_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<StatusCode, HttpError> {
    state.db.delete_relayer(&relayer_id).await?;

    invalidate_relayer_cache(&state.cache, &relayer_id).await;
    state.transactions_queues.lock().await.delete_queue(&relayer_id).await;
    Ok(StatusCode::NO_CONTENT)
}
