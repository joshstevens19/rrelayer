use std::sync::Arc;

use axum::{extract::State, Json};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    network::{cache::get_networks_cache, types::Network},
};

/// Returns a list of all networks
pub async fn networks(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Network>>, HttpError> {
    let networks = get_networks_cache(&state.cache).await;

    Ok(Json(networks))
}
