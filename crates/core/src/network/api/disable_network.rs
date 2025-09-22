use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::shared::{bad_request, HttpError};
use crate::{
    app_state::AppState,
    network::{cache::invalidate_disabled_networks_cache, types::ChainId},
};

/// Disables the network with the specified chain ID.
pub async fn disable_network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> Result<StatusCode, HttpError> {
    let exists = state.db.network_exists(chain_id).await?;
    if !exists {
        return Err(bad_request("Network does not exist".to_string()));
    }

    state.db.disable_network(chain_id).await?;
    invalidate_disabled_networks_cache(&state.cache).await;
    Ok(StatusCode::OK)
}
