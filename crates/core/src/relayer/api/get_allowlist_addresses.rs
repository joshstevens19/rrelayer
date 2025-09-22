use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::relayer::get_relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    relayer::types::RelayerId,
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
};

#[derive(Debug, Deserialize)]
pub struct GetAllowlistAddressesQuery {
    limit: u32,
    offset: u32,
}

/// Retrieves the allowlist addresses for a relayer.
pub async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetAllowlistAddressesQuery>,
) -> Result<Json<PagingResult<EvmAddress>>, HttpError> {
    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    if exists {
        let result = state
            .db
            .relayer_get_allowlist_addresses(
                &relayer_id,
                &PagingContext::new(query.limit, query.offset),
            )
            .await?;

        Ok(Json(result))
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
