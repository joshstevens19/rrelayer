use std::sync::Arc;

use axum::{http::StatusCode, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{app_state::AppState, authentication::basic_auth::Authenticated};

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub authenticated: bool,
    pub message: String,
}

/// Simple endpoint to verify basic auth credentials work.
///
/// This endpoint can be used to test that basic auth is working correctly.
/// If you can access this endpoint, your credentials are valid.
///
/// # Returns
/// * `Ok(Json<StatusResponse>)` - Authentication status if basic auth succeeds
/// * `Err(StatusCode::UNAUTHORIZED)` - If basic auth fails
async fn status(_auth: Authenticated) -> Result<Json<StatusResponse>, StatusCode> {
    Ok(Json(StatusResponse {
        authenticated: true,
        message: "Basic authentication successful".to_string(),
    }))
}

/// Creates a simple authentication router for basic auth testing.
///
/// This provides a simple endpoint to verify basic auth is working.
///
/// # Returns
/// * `Router<Arc<AppState>>` - A configured router with basic auth test endpoint:
///   - GET /status - Returns authentication status (requires basic auth)
pub fn create_basic_auth_routes() -> Router<Arc<AppState>> {
    Router::new().route("/status", get(status))
}
