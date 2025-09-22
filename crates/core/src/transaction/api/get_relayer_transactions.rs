use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{
    app_state::AppState,
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingQuery, PagingResult},
    transaction::types::Transaction,
};

/// API endpoint to retrieve all transactions for a specific relayer.
///
/// Returns a paginated list of transactions associated with the given relayer.
///
/// # Arguments
/// * `state` - The application state containing the database connection
/// * `relayer_id` - The relayer ID path parameter
/// * `paging` - Query parameters for pagination (limit, offset)
/// * `headers` - HTTP headers (for future API key validation)
/// * `auth_guard` - Authentication guard for access control
///
/// # Returns
/// * `Ok(Json<PagingResult<Transaction>>)` - Paginated list of transactions
/// * `Err(StatusCode)` - INTERNAL_SERVER_ERROR if database query fails
pub async fn get_relayer_transactions(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(paging): Query<PagingQuery>,
) -> Result<Json<PagingResult<Transaction>>, StatusCode> {
    let paging_context = PagingContext::new(paging.limit, paging.offset);

    state
        .db
        .get_transactions_for_relayer(&relayer_id, &paging_context)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
