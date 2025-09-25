use std::sync::Arc;

use crate::relayer::get_relayer;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::types::RelayerId, shared::common_types::EvmAddress};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};

/// Retrieves the allowlist addresses for a relayer.
pub async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<Vec<EvmAddress>>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer = get_relayer(&state.db, &state.cache, &relayer_id).await?;
    if let Some(relayer) = relayer {
        state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)?;

        Ok(Json(state.restricted_addresses(&relayer.address, &relayer.chain_id)))
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
