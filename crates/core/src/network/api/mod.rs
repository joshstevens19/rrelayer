use std::sync::Arc;

use axum::{
    routing::{get, put},
    Router,
};

use crate::app_state::AppState;

pub mod disable_network;
pub mod disabled_networks;
pub mod enable_network;
pub mod enabled_networks;
pub mod networks;

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
        .route("/", get(networks::networks))
        .route("/enabled", get(enabled_networks::enabled_networks))
        .route("/disabled", get(disabled_networks::disabled_networks))
        .route("/disable/:chain_id", put(disable_network::disable_network))
        .route("/enable/:chain_id", put(enable_network::enable_network))
}
