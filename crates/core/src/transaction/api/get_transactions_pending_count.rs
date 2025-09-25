use crate::relayer::get_relayer;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::RelayerId};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

/// API endpoint to get the count of pending transactions for a relayer.
pub async fn get_transactions_pending_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<usize>, HttpError> {
    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await?
        .ok_or(not_found("Relayer could not be found".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)?;

    let count =
        state.transactions_queues.lock().await.pending_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
