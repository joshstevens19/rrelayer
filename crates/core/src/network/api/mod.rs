use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware::from_fn,
    routing::{get, put},
    Json, Router,
};

use super::{
    cache::{
        get_disabled_networks_cache, get_enabled_networks_cache, get_networks_cache,
        invalidate_disabled_networks_cache, invalidate_enabled_networks_cache,
        set_disabled_networks_cache, set_enabled_networks_cache, set_networks_cache,
    },
    types::{ChainId, Network, NetworksFilterState},
};
use crate::{
    app_state::AppState,
    authentication::guards::{admin_jwt_guard, read_only_or_above_jwt_guard},
    rrelayer_error,
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
async fn networks(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Network>>, StatusCode> {
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
async fn enabled_networks(
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
async fn disabled_networks(
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

/// HTTP handler for disabling a specific network.
///
/// Disables the network with the specified chain ID and invalidates
/// related cache entries to ensure consistency.
///
/// # Arguments
/// * `state` - Application state containing database connection and cache
/// * `chain_id` - Chain ID of the network to disable
///
/// # Returns
/// * `StatusCode::CREATED` - If network was successfully disabled
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn disable_network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> StatusCode {
    match state.db.disable_network(chain_id).await {
        Ok(_) => {
            invalidate_disabled_networks_cache(&state.cache).await;
            StatusCode::CREATED
        }
        Err(e) => {
            rrelayer_error!("Failed to disable network: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// HTTP handler for enabling a specific network.
///
/// Enables the network with the specified chain ID and invalidates
/// related cache entries to ensure consistency.
///
/// # Arguments
/// * `state` - Application state containing database connection and cache
/// * `chain_id` - Chain ID of the network to enable
///
/// # Returns
/// * `StatusCode::CREATED` - If network was successfully enabled
/// * `StatusCode::BAD_REQUEST` - If database operation fails
async fn enable_network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> StatusCode {
    match state.db.enable_network(chain_id).await {
        Ok(_) => {
            invalidate_enabled_networks_cache(&state.cache).await;
            StatusCode::CREATED
        }
        Err(_) => StatusCode::BAD_REQUEST,
    }
}

/// Creates and configures the network API routes.
///
/// Sets up all network-related HTTP endpoints with appropriate middleware:
/// - GET /: Returns all networks (requires read-only access)
/// - GET /enabled: Returns enabled networks (requires read-only access)  
/// - GET /disabled: Returns disabled networks (requires read-only access)
/// - PUT /disable/:chain_id: Disables a network (requires admin access)
/// - PUT /enable/:chain_id: Enables a network (requires admin access)
///
/// # Returns
/// * `Router<Arc<AppState>>` - Configured router with all network endpoints
pub fn create_network_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(networks))
        .route("/enabled", get(enabled_networks))
        .route("/disabled", get(disabled_networks))
        .route_layer(from_fn(read_only_or_above_jwt_guard))
        .route("/disable/:chain_id", put(disable_network))
        .route("/enable/:chain_id", put(enable_network))
        .route_layer(from_fn(admin_jwt_guard))
}
