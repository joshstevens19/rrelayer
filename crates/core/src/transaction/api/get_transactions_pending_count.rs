use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::shared::HttpError;
use crate::{app_state::AppState, relayer::RelayerId};

/// API endpoint to get the count of pending transactions for a relayer.
pub async fn get_transactions_pending_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<usize>, HttpError> {
    let count =
        state.transactions_queues.lock().await.pending_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
