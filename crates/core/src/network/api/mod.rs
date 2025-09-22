use std::sync::Arc;

use axum::{
    routing::{get, put},
    Router,
};

use crate::app_state::AppState;

mod disable_network;
mod disabled_networks;
mod enable_network;
mod enabled_networks;
mod networks;

pub fn create_network_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(networks::networks))
        .route("/enabled", get(enabled_networks::enabled_networks))
        .route("/disabled", get(disabled_networks::disabled_networks))
        .route("/disable/:chain_id", put(disable_network::disable_network))
        .route("/enable/:chain_id", put(enable_network::enable_network))
}
