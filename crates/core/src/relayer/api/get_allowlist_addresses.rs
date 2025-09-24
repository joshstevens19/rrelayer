use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::relayer::get_relayer;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, relayer::types::RelayerId, shared::common_types::EvmAddress};

/// Retrieves the allowlist addresses for a relayer.
pub async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<Vec<EvmAddress>>, HttpError> {
    let relayer = get_relayer(&state.db, &state.cache, &relayer_id).await?;
    if let Some(relayer) = relayer {
        Ok(Json(state.restricted_addresses(&relayer.address, &relayer.chain_id)))
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
