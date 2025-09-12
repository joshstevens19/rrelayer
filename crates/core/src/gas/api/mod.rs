use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

pub mod get_gas_price;

/// Creates and configures the gas-related HTTP routes.
///
/// Sets up routes for gas price retrieval with authentication middleware
/// requiring read-only or above permissions.
///
/// # Returns
/// * `Router<Arc<AppState>>` - Configured router with gas price endpoints
pub fn create_gas_routes() -> Router<Arc<AppState>> {
    Router::new().route("/price/:chain_id", get(get_gas_price::get_gas_price))
}
