use std::env;

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};

/// Authentication middleware that validates API secret keys.
///
/// Checks for the presence of the `x-api-secret-access-key` header
/// and validates it against the `API_SECRET` environment variable.
/// If authentication succeeds, inserts an `Authenticated` marker
/// into the request extensions for use by downstream handlers.
///
/// # Arguments
/// * `req` - The incoming HTTP request
/// * `next` - The next middleware or handler in the chain
///
/// # Returns
/// * `Ok(Response)` - If authentication succeeds, passes request to next handler
/// * `Err(StatusCode::UNAUTHORIZED)` - If authentication fails
///
/// # Headers
/// Expects the `x-api-secret-access-key` header with a valid API secret.
pub async fn auth_middleware(mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let authenticated = req
        .headers()
        .get("x-api-secret-access-key")
        .and_then(|value| value.to_str().ok())
        .map(validate_secret)
        .unwrap_or(false);

    if authenticated {
        req.extensions_mut().insert(Authenticated);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Validates a secret against the API_SECRET environment variable.
///
/// # Arguments
/// * `secret` - The secret to validate
///
/// # Returns
/// * `true` - If the secret matches the API_SECRET environment variable
/// * `false` - If the secret doesn't match or API_SECRET is not set
fn validate_secret(secret: &str) -> bool {
    env::var("API_SECRET").map(|value| value == secret).unwrap_or(false)
}

/// Marker type indicating that a request has been authenticated.
///
/// This type is inserted into request extensions by the auth middleware
/// and can be extracted by handlers to ensure authentication has occurred.
/// 
/// # Example
/// ```rust,ignore
/// async fn protected_handler(auth: Authenticated) -> &'static str {
///     "This handler requires authentication"
/// }
/// ```
#[derive(Clone)]
pub struct Authenticated;

#[async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    /// Extracts authentication marker from request extensions.
    ///
    /// # Returns
    /// * `Ok(Authenticated)` - If the request has been authenticated
    /// * `Err(StatusCode::UNAUTHORIZED)` - If authentication marker is missing
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Authenticated>()
            .map(|_| Authenticated)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}
