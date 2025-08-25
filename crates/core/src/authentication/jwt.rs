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

fn get_secret_key(token_type: JwtTokenType) -> Result<String, JwtError> {
    match token_type {
        JwtTokenType::Access => env::var("ACCESS_JWT_SECRET_KEY")
            .map_err(|_| JwtError::MissingEnvVar("ACCESS_JWT_SECRET_KEY".to_string())),
        JwtTokenType::Refresh => env::var("REFRESH_JWT_SECRET_KEY")
            .map_err(|_| JwtError::MissingEnvVar("REFRESH_JWT_SECRET_KEY".to_string())),
    }
}

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

pub fn validate_token(token_type: JwtTokenType, token: &str) -> Result<JwtClaims, JwtError> {
    let validation = Validation::new(Algorithm::HS256);

    let decoded_access_token = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(get_secret_key(token_type)?.as_ref()),
        &validation,
    )?;

    Ok(decoded_access_token.claims)
}

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
