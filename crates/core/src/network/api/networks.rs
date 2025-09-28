use std::sync::Arc;

use crate::shared::HttpError;
use crate::{app_state::AppState, network::types::Network};
use axum::http::HeaderMap;
use axum::{extract::State, Json};

/// Returns a list of all networks
pub async fn networks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<Network>>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    let all_networks: Vec<Network> =
        state.network_configs.iter().cloned().map(|n| n.into()).collect();

    Ok(Json(all_networks))
}
