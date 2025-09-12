use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    app_state::AppState,
    network::{
        cache::{get_networks_cache, set_networks_cache},
        types::{Network, NetworksFilterState},
    },
};

/// HTTP handler for retrieving all networks.
///
/// Returns a list of all networks (enabled and disabled) from cache if available,
/// otherwise fetches from database and caches the result.
///
/// # Arguments
/// * `state` - Application state containing database connection and cache
///
/// # Returns
/// * `Ok(Json<Vec<Network>>)` - List of all networks
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If database query fails
pub async fn networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    let networks = state
        .db
        .get_networks(NetworksFilterState::All)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    set_networks_cache(&state.cache, &networks).await;
    Ok(Json(networks))
}
