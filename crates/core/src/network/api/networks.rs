use std::sync::Arc;

use axum::{extract::State, Json};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    network::{
        cache::{get_networks_cache, set_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// Returns a list of all networks (enabled and disabled)
pub async fn networks(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Network>>, HttpError> {
    if let Some(cached_result) = get_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let networks = state.db.get_networks(NetworksFilterState::All).await?;

    set_networks_cache(&state.cache, &networks).await;
    Ok(Json(networks))
}
