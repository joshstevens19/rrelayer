use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

mod networks;

pub fn create_network_routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(networks::networks))
}
