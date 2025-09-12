use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::authentication::basic_auth::Authenticated;

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
pub async fn status(_auth: Authenticated) -> Result<Json<StatusResponse>, StatusCode> {
    Ok(Json(StatusResponse {
        authenticated: true,
        message: "Basic authentication successful".to_string(),
    }))
}
