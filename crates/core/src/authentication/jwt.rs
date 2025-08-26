use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use thiserror::Error;

use super::types::{AccessToken, JwtClaims, JwtRole, JwtTokenType, RefreshToken, TokenPair};
use crate::shared::common_types::EvmAddress;

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("JWT error: {0}")]
    JsonWebToken(#[from] jsonwebtoken::errors::Error),
    #[error("Environment variable {0} not set")]
    MissingEnvVar(String),
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
}

// TODO! FIX THIS FOR NOW MAKE IT LONG LIVED TOKEN SHOULD BE 5 MINUTES
const ACCESS_EXP_SECONDS: u64 = 300000000; // 5 minutes
const REFRESH_EXP_SECONDS: u64 = 3600; // 1 hour

/// Retrieves the JWT secret key from environment variables.
///
/// Gets the appropriate secret key based on the token type from environment variables.
/// Access tokens use ACCESS_JWT_SECRET_KEY and refresh tokens use REFRESH_JWT_SECRET_KEY.
///
/// # Arguments
/// * `token_type` - The type of JWT token (Access or Refresh)
///
/// # Returns
/// * `Ok(String)` - The secret key if found in environment variables
/// * `Err(JwtError)` - If the required environment variable is not set
fn get_secret_key(token_type: JwtTokenType) -> Result<String, JwtError> {
    match token_type {
        JwtTokenType::Access => env::var("ACCESS_JWT_SECRET_KEY")
            .map_err(|_| JwtError::MissingEnvVar("ACCESS_JWT_SECRET_KEY".to_string())),
        JwtTokenType::Refresh => env::var("REFRESH_JWT_SECRET_KEY")
            .map_err(|_| JwtError::MissingEnvVar("REFRESH_JWT_SECRET_KEY".to_string())),
    }
}

/// Creates a pair of JWT tokens (access and refresh) for authentication.
///
/// Generates both an access token and refresh token for the given EVM address and role.
/// The access token has a very long expiration (currently set for testing), while the
/// refresh token expires after 1 hour.
///
/// # Arguments
/// * `address` - The EVM address to include in the token claims
/// * `role` - The user role to include in the token claims
///
/// # Returns
/// * `Ok(TokenPair)` - A pair containing both access and refresh tokens
/// * `Err(JwtError)` - If token creation fails due to missing environment variables or encoding errors
pub fn create_auth_tokens(address: &EvmAddress, role: JwtRole) -> Result<TokenPair, JwtError> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let access_claims = JwtClaims::new(
        address.to_owned(),
        role.clone(),
        (now + ACCESS_EXP_SECONDS) as usize,
        now as usize,
    );

    let refresh_claims = JwtClaims::new(
        address.to_owned(),
        role,
        (now + REFRESH_EXP_SECONDS) as usize,
        now as usize,
    );

    let access_token: AccessToken = encode(
        &Header::new(Algorithm::HS256),
        &access_claims,
        &EncodingKey::from_secret(get_secret_key(JwtTokenType::Access)?.as_ref()),
    )?;

    let refresh_token: RefreshToken = encode(
        &Header::new(Algorithm::HS256),
        &refresh_claims,
        &EncodingKey::from_secret(get_secret_key(JwtTokenType::Refresh)?.as_ref()),
    )?;

    Ok(TokenPair { access_token, refresh_token })
}

/// Validates a JWT token and extracts its claims.
///
/// Decodes and validates a JWT token using the appropriate secret key based on token type.
/// Performs signature verification and expiration checks.
///
/// # Arguments
/// * `token_type` - The type of JWT token (Access or Refresh)
/// * `token` - The JWT token string to validate
///
/// # Returns
/// * `Ok(JwtClaims)` - The decoded JWT claims if validation succeeds
/// * `Err(JwtError)` - If validation fails due to invalid signature, expiration, or other JWT errors
pub fn validate_token(token_type: JwtTokenType, token: &str) -> Result<JwtClaims, JwtError> {
    let validation = Validation::new(Algorithm::HS256);

    let decoded_access_token = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(get_secret_key(token_type)?.as_ref()),
        &validation,
    )?;

    Ok(decoded_access_token.claims)
}

/// Validates a JWT token and verifies it has a specific role.
///
/// Decodes and validates a JWT token, then checks that the token's role claim
/// matches the required role exactly. Returns the EVM address if validation succeeds.
///
/// # Arguments
/// * `token_type` - The type of JWT token (Access or Refresh)
/// * `role` - The required role that the token must have
/// * `token` - The JWT token string to validate
///
/// # Returns
/// * `Ok(EvmAddress)` - The EVM address from the token if validation and role check succeed
/// * `Err(JwtError)` - If validation fails or the role doesn't match
pub fn validate_token_with_role(
    token_type: JwtTokenType,
    role: JwtRole,
    token: &str,
) -> Result<EvmAddress, JwtError> {
    let validation = Validation::new(Algorithm::HS256);

    let decoded_access_token = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(get_secret_key(token_type)?.as_ref()),
        &validation,
    )?;

    if decoded_access_token.claims.role != role {
        return Err(JwtError::JsonWebToken(jsonwebtoken::errors::ErrorKind::InvalidToken.into()));
    }

    Ok(decoded_access_token.claims.sub)
}

/// Validates a JWT token and verifies it has one of the allowed roles.
///
/// Decodes and validates a JWT token, then checks that the token's role claim
/// is included in the list of allowed roles. Returns the EVM address if validation succeeds.
///
/// # Arguments
/// * `token_type` - The type of JWT token (Access or Refresh)
/// * `roles` - A vector of allowed roles that the token can have
/// * `token` - The JWT token string to validate
///
/// # Returns
/// * `Ok(EvmAddress)` - The EVM address from the token if validation and role check succeed
/// * `Err(JwtError)` - If validation fails or the role is not in the allowed list
pub fn validate_token_includes_role(
    token_type: JwtTokenType,
    roles: Vec<JwtRole>,
    token: &str,
) -> Result<EvmAddress, JwtError> {
    let validation = Validation::new(Algorithm::HS256);

    let decoded_access_token = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(get_secret_key(token_type)?.as_ref()),
        &validation,
    )?;

    if !roles.contains(&decoded_access_token.claims.role) {
        return Err(JwtError::JsonWebToken(jsonwebtoken::errors::ErrorKind::InvalidToken.into()));
    }

    Ok(decoded_access_token.claims.sub)
}
