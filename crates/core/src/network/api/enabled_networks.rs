use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    app_state::AppState,
    network::{
        cache::{get_enabled_networks_cache, set_enabled_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// HTTP handler for retrieving enabled networks only.
///
/// Returns a list of enabled networks from cache if available,
/// otherwise fetches from database and caches the result.
///
/// # Arguments
/// * `state` - Application state containing database connection and cache
///
/// # Returns
/// * `Ok(Json<Vec<Network>>)` - List of enabled networks
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If database query fails
pub async fn enabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_enabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let enabled_networks = state
        .db
        .get_networks(NetworksFilterState::Enabled)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    set_enabled_networks_cache(&state.cache, &enabled_networks).await;
    Ok(Json(enabled_networks))
}
