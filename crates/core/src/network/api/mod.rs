use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

mod get_gas_price;
mod networks;

pub fn create_network_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(networks::networks))
        .route("/gas/price/:chain_id", get(get_gas_price::get_gas_price))
}
