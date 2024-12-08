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
};

async fn networks(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    match state.db.get_networks(NetworksFilterState::All).await {
        Ok(networks) => {
            set_networks_cache(&state.cache, &networks).await;
            Ok(Json(networks))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn enabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_enabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    match state.db.get_networks(NetworksFilterState::Enabled).await {
        Ok(enabled_networks) => {
            set_enabled_networks_cache(&state.cache, &enabled_networks).await;
            Ok(Json(enabled_networks))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn disabled_networks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Network>>, StatusCode> {
    if let Some(cached_result) = get_disabled_networks_cache(&state.cache).await {
        return Ok(Json(cached_result));
    }

    match state.db.get_networks(NetworksFilterState::Disabled).await {
        Ok(disabled_networks) => {
            set_disabled_networks_cache(&state.cache, &disabled_networks).await;
            Ok(Json(disabled_networks))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn disable_network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> StatusCode {
    match state.db.disable_network(chain_id).await {
        Ok(_) => {
            invalidate_disabled_networks_cache(&state.cache).await;
            StatusCode::CREATED
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

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
