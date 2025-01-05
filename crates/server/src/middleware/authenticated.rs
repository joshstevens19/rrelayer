use std::env;

use axum::{
    async_trait,
    body::Body,
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};

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

fn validate_secret(secret: &str) -> bool {
    env::var("API_SECRET").map(|value| value == secret).unwrap_or(false)
}

#[derive(Clone)]
pub struct Authenticated;

#[async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Authenticated>()
            .map(|_| Authenticated)
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}
