use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

use crate::shared::HttpError;
use crate::{app_state::AppState, relayer::RelayerId};

/// API endpoint to get the count of in-mempool transactions for a relayer.
pub async fn get_transactions_inmempool_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<usize>, HttpError> {
    let count =
        state.transactions_queues.lock().await.inmempool_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
