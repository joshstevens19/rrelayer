use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    network::{cache::invalidate_disabled_networks_cache, types::ChainId},
    rrelayer_error,
};

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
pub async fn disable_network(
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
