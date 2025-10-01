use std::sync::Arc;

use axum::http::HeaderMap;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::shared::{bad_request, HttpError};
use crate::{
    app_state::AppState,
    network::ChainId,
    provider::chain_enabled,
    relayer::types::Relayer,
    shared::common_types::{PagingContext, PagingResult},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct GetRelayersQuery {
    #[serde(rename = "chainId", skip_serializing_if = "Option::is_none")]
    chain_id: Option<ChainId>,
    limit: u32,
    offset: u32,
}

/// Retrieves a paginated list of relayers, optionally filtered by chain ID.
pub async fn get_relayers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetRelayersQuery>,
    headers: HeaderMap,
) -> Result<Json<PagingResult<Relayer>>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;
    match query.chain_id {
        Some(chain_id) => {
            if !chain_enabled(&state.evm_providers, &chain_id) {
                return Err(bad_request("Chain is not enabled".to_string()));
            }

            let result = state
                .db
                .get_relayers_for_chain(&chain_id, &PagingContext::new(query.limit, query.offset))
                .await?;

            Ok(Json(result))
        }
        None => {
            let result =
                state.db.get_relayers(&PagingContext::new(query.limit, query.offset)).await?;

            Ok(Json(result))
        }
    }
}
