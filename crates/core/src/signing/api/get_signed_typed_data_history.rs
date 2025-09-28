use crate::relayer::get_relayer;
use crate::shared::{not_found, HttpError};
use crate::signing::db::SignedTypedDataHistory;
use crate::{
    app_state::AppState,
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingResult},
};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct GetSigningHistoryQuery {
    pub limit: u32,
    pub offset: u32,
}

/// Retrieves the history of signed typed data messages with optional filtering.
pub async fn get_signed_typed_data_history(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetSigningHistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<PagingResult<SignedTypedDataHistory>>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await?
        .ok_or(not_found("Relayer could not be found".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)?;

    let paging_context = PagingContext::new(query.limit, query.offset);

    let result = state.db.get_signed_typed_data_history(&relayer_id, &paging_context).await?;

    Ok(Json(result))
}
