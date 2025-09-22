use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingQuery, PagingResult},
    transaction::types::Transaction,
};

/// API endpoint to retrieve all transactions for a specific relayer.
pub async fn get_relayer_transactions(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(paging): Query<PagingQuery>,
) -> Result<Json<PagingResult<Transaction>>, HttpError> {
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        let paging_context = PagingContext::new(paging.limit, paging.offset);

        let result = state.db.get_transactions_for_relayer(&relayer_id, &paging_context).await?;

        Ok(Json(result))
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
