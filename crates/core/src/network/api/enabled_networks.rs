use std::sync::Arc;

use axum::{extract::State, Json};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    network::{
        cache::{get_enabled_networks_cache, set_enabled_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// Returns a list of enabled networks.
pub async fn enabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, HttpError> {
    if let Some(cached_result) = get_enabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let enabled_networks = state.db.get_networks(NetworksFilterState::Enabled).await?;

    set_enabled_networks_cache(&state.cache, &enabled_networks).await;
    Ok(Json(enabled_networks))
}
