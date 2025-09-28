use std::sync::Arc;

use crate::network::ChainId;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, network::types::Network};
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::{extract::State, Json};

/// Returns a network
pub async fn network(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    headers: HeaderMap,
) -> Result<Json<Network>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    let networks = state.network_configs.iter().cloned().collect::<Vec<_>>();
    let network = networks.into_iter().find(|n| n.chain_id == chain_id);
    if let Some(network) = network {
        return Ok(Json(network.into()));
    }

    Err(not_found(
        "Could not find network are you sure its enabled in the rrelayer.yaml?".to_string(),
    ))
}
