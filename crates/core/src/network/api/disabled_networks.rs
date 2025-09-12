use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    app_state::AppState,
    network::{
        cache::{get_disabled_networks_cache, set_disabled_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// HTTP handler for retrieving disabled networks only.
///
/// Returns a list of disabled networks from cache if available,
/// otherwise fetches from database and caches the result.
///
/// # Arguments
/// * `state` - Application state containing database connection and cache
///
/// # Returns
/// * `Ok(Json<Vec<Network>>)` - List of disabled networks
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If database query fails
pub async fn disabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_disabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let disabled_networks = state
        .db
        .get_networks(NetworksFilterState::Disabled)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    set_disabled_networks_cache(&state.cache, &disabled_networks).await;
    Ok(Json(disabled_networks))
}
