use std::env;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
};
use base64::{engine::general_purpose, Engine as _};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BasicAuthError {
    #[error("Missing Authorization header")]
    MissingAuthHeader,
    #[error("Invalid Authorization header format")]
    InvalidHeaderFormat,
    #[error("Invalid base64 encoding")]
    InvalidBase64,
    #[error("Invalid credentials format")]
    InvalidCredentialsFormat,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Missing server credentials configuration")]
    MissingServerCredentials,
}

/// Simple basic auth credentials extracted from HTTP headers
#[derive(Debug, Clone)]
pub struct BasicAuthCredentials {
    pub username: String,
    pub password: String,
}

impl BasicAuthCredentials {
    /// Extracts basic auth credentials from HTTP headers
    pub fn from_headers(headers: &HeaderMap) -> Result<Self, BasicAuthError> {
        let auth_header = headers
            .get("Authorization")
            .ok_or(BasicAuthError::MissingAuthHeader)?
            .to_str()
            .map_err(|_| BasicAuthError::InvalidHeaderFormat)?;

        if !auth_header.starts_with("Basic ") {
            return Err(BasicAuthError::InvalidHeaderFormat);
        }

        let base64_credentials = &auth_header[6..]; // Skip "Basic "
        let decoded = general_purpose::STANDARD
            .decode(base64_credentials)
            .map_err(|_| BasicAuthError::InvalidBase64)?;

        let credentials_str =
            String::from_utf8(decoded).map_err(|_| BasicAuthError::InvalidBase64)?;

        let parts: Vec<&str> = credentials_str.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(BasicAuthError::InvalidCredentialsFormat);
        }

        Ok(BasicAuthCredentials { username: parts[0].to_string(), password: parts[1].to_string() })
    }

    /// Validates credentials against server configuration
    pub fn validate(&self) -> Result<(), BasicAuthError> {
        let server_username = env::var("RRELAYER_AUTH_USERNAME")
            .map_err(|_| BasicAuthError::MissingServerCredentials)?;
        let server_password = env::var("RRELAYER_AUTH_PASSWORD")
            .map_err(|_| BasicAuthError::MissingServerCredentials)?;

        if self.username == server_username && self.password == server_password {
            Ok(())
        } else {
            Err(BasicAuthError::InvalidCredentials)
        }
    }
}

/// Simple authenticated marker - just indicates that basic auth passed
#[derive(Debug)]
pub struct Authenticated;

/// Basic auth extractor that validates server-wide credentials
#[async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let credentials = BasicAuthCredentials::from_headers(&parts.headers)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        credentials.validate().map_err(|_| StatusCode::UNAUTHORIZED)?;

        Ok(Authenticated)
    }
}

/// Validates basic auth credentials from headers
pub fn validate_basic_auth(headers: &HeaderMap) -> Result<(), BasicAuthError> {
    let credentials = BasicAuthCredentials::from_headers(headers)?;
    credentials.validate()
}
