use std::sync::Arc;

use axum::{extract::State, Json};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    network::{
        cache::{get_disabled_networks_cache, set_disabled_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// Returns a list of disabled networks.
pub async fn disabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, HttpError> {
    if let Some(cached_result) = get_disabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let disabled_networks = state.db.get_networks(NetworksFilterState::Disabled).await?;

    set_disabled_networks_cache(&state.cache, &disabled_networks).await;
    Ok(Json(disabled_networks))
}
