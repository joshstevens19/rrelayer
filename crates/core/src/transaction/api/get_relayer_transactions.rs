use crate::relayer::{get_relayer, relayer_exists};
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingQuery, PagingResult},
    transaction::types::Transaction,
};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

/// API endpoint to retrieve all transactions for a specific relayer.
pub async fn get_relayer_transactions(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(paging): Query<PagingQuery>,
    headers: HeaderMap,
) -> Result<Json<PagingResult<Transaction>>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await?
        .ok_or(not_found("Relayer could not be found".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)?;

    let paging_context = PagingContext::new(paging.limit, paging.offset);

    let result = state.db.get_transactions_for_relayer(&relayer_id, &paging_context).await?;

    Ok(Json(result))
}
