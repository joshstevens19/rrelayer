use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};

use crate::authentication::basic_auth::validate_basic_auth;

/// Simple basic auth middleware that validates server-wide credentials
///
/// This middleware validates that the request contains valid basic auth credentials
/// matching the server's configured username and password from environment variables.
pub async fn simple_basic_auth_guard(
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Validate basic auth credentials
    validate_basic_auth(&req.headers()).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // If authentication passes, continue to the next handler
    Ok(next.run(req).await)
}
