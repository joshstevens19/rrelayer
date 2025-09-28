use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use tracing::error;

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
        parts.extensions.get::<Authenticated>().map(|_| Authenticated).ok_or_else(|| {
            error!(
                "Authentication marker missing from request extensions for path: {}",
                parts.uri.path()
            );
            StatusCode::UNAUTHORIZED
        })
    }
}
