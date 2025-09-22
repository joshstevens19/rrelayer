use std::sync::Arc;

use crate::network::cache::invalidate_enabled_networks_cache;
use crate::shared::{bad_request, HttpError};
use crate::{
    app_state::AppState,
    network::{
        cache::set_networks_cache,
        types::{ChainId, NetworksFilterState},
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use tracing::error;

/// Enables the network with the specified chain ID.
pub async fn enable_network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> Result<StatusCode, HttpError> {
    let exists = state.db.network_exists(chain_id).await?;
    if !exists {
        return Err(bad_request("Network does not exist".to_string()));
    }

    state.db.enable_network(chain_id).await?;

    let postgres_client = state.db.clone();
    let cache = state.cache.clone();
    invalidate_enabled_networks_cache(&cache).await;
    match postgres_client.get_networks(NetworksFilterState::All).await {
        Ok(networks) => {
            set_networks_cache(&cache, &networks).await;
        }
        Err(e) => {
            error!("Failed to refresh networks cache after enabling network {}: {}", chain_id, e);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
