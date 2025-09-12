use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    app_state::AppState,
    relayer::{get_relayer, types::RelayerId},
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
};

#[derive(Debug, Deserialize)]
pub struct GetAllowlistAddressesQuery {
    limit: u32,
    offset: u32,
}

/// Retrieves the allowlist addresses for a relayer.
///
/// This endpoint returns a paginated list of Ethereum addresses that are allowed
/// to use this relayer for transaction processing when allowlist mode is enabled.
///
/// # Arguments
/// * `state` - Application state containing database connections
/// * `auth_guard` - Authentication guard for access control
/// * `relayer_id` - The unique identifier of the relayer
/// * `query` - Query parameters for pagination (limit and offset)
///
/// # Returns
/// * `Ok(Json<PagingResult<EvmAddress>>)` - Paginated list of allowlisted addresses
/// * `Err(StatusCode::UNAUTHORIZED)` - If authentication fails
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If retrieval fails
pub async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetAllowlistAddressesQuery>,
) -> Result<Json<PagingResult<EvmAddress>>, StatusCode> {
    get_relayer(&state.db, &state.cache, &relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    state
        .db
        .relayer_get_allowlist_addresses(
            &relayer_id,
            &PagingContext::new(query.limit, query.offset),
        )
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
