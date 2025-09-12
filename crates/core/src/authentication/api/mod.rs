use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

pub mod status;

/// Creates a simple authentication router for basic auth testing.
///
/// This provides a simple endpoint to verify basic auth is working.
///
/// # Returns
/// * `Router<Arc<AppState>>` - A configured router with basic auth test endpoint:
///   - GET /status - Returns authentication status (requires basic auth)
pub fn create_basic_auth_routes() -> Router<Arc<AppState>> {
    Router::new().route("/status", get(status::status))
}
