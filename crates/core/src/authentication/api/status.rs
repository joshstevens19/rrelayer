use crate::app_state::AppState;
use crate::shared::HttpError;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub authenticated: bool,
    pub message: String,
}

/// Simple endpoint to verify basic auth credentials work.
pub async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<StatusResponse>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    Ok(Json(StatusResponse {
        authenticated: true,
        message: "Basic authentication successful".to_string(),
    }))
}
