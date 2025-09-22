use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::signing::db::SignedTypedDataHistory;
use crate::{
    app_state::AppState,
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingResult},
};

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
) -> Result<Json<PagingResult<SignedTypedDataHistory>>, HttpError> {
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        let paging_context = PagingContext::new(query.limit, query.offset);

        let result = state.db.get_signed_typed_data_history(&relayer_id, &paging_context).await?;

        Ok(Json(result))
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
