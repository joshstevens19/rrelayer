use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    network::{cache::invalidate_enabled_networks_cache, types::ChainId},
};

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
pub async fn enable_network(
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
