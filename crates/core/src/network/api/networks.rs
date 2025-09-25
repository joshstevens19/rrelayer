use std::sync::Arc;

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    network::{cache::get_networks_cache, types::Network},
};
use axum::http::HeaderMap;
use axum::{extract::State, Json};

/// Returns a list of all networks
pub async fn networks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<Network>>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    let networks = get_networks_cache(&state.cache).await;

    Ok(Json(networks))
}
