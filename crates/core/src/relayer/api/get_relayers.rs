use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    app_state::AppState,
    network::ChainId,
    provider::chain_enabled,
    relayer::types::Relayer,
    shared::common_types::{PagingContext, PagingResult},
};

#[derive(Debug, Deserialize)]
pub struct GetRelayersQuery {
    chain_id: Option<ChainId>,
    limit: u32,
    offset: u32,
}

/// Retrieves a paginated list of relayers, optionally filtered by chain ID.
///
/// This endpoint returns a list of all relayers with optional filtering by blockchain network.
/// Results are paginated using limit and offset parameters.
///
/// # Arguments
/// * `state` - Application state containing database connections
/// * `query` - Query parameters including optional chain_id, limit, and offset
///
/// # Returns
/// * `Ok(Json<PagingResult<Relayer>>)` - Paginated list of relayers
/// * `Err(StatusCode)` - HTTP error code if retrieval fails
pub async fn get_relayers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetRelayersQuery>,
) -> Result<Json<PagingResult<Relayer>>, StatusCode> {
    match query.chain_id {
        Some(chain_id) => {
            if !chain_enabled(&state.evm_providers, &chain_id) {
                return Err(StatusCode::BAD_REQUEST);
            }

            state
                .db
                .get_relayers_for_chain(&chain_id, &PagingContext::new(query.limit, query.offset))
                .await
                .map(Json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => state
            .db
            .get_relayers(&PagingContext::new(query.limit, query.offset))
            .await
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
    }
}
