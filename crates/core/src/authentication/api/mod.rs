use std::sync::Arc;

use alloy::signers;
use axum::{extract::State, http::StatusCode, middleware::from_fn, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    cache::{
        get_authentication_challenge_cache, invalidate_authentication_challenge_cache,
        set_authentication_challenge_cache,
    },
    jwt::{create_auth_tokens, validate_token},
    types::{JwtTokenType, RefreshToken, TokenPair},
};
use crate::{
    app_state::AppState, authentication::guards::refresh_jwt_token_guard,
    shared::common_types::EvmAddress,
};

#[derive(Debug, Deserialize)]
struct GenerateSecretRequest {
    pub address: EvmAddress,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateSecretResult {
    pub id: Uuid,
    pub challenge: String,
    pub address: EvmAddress,
}

/// Generates an authentication challenge for wallet signature verification.
///
/// Creates a unique challenge string that includes a welcome message, the user's wallet address,
/// and a random nonce. The challenge is cached for later verification during authentication.
/// This endpoint requires the user to exist in the database.
///
/// # Arguments
/// * `state` - The application state containing database and cache connections
/// * `secret_request` - The request containing the EVM address to generate a challenge for
///
/// # Returns
/// * `Ok(Json<GenerateSecretResult>)` - The generated challenge with ID, message, and address
/// * `Err(StatusCode)` - UNAUTHORIZED if user doesn't exist, INTERNAL_SERVER_ERROR for database errors
async fn generate_auth_secret(
    State(state): State<Arc<AppState>>,
    Json(secret_request): Json<GenerateSecretRequest>,
) -> Result<Json<GenerateSecretResult>, StatusCode> {
    state
        .db
        .get_user(&secret_request.address)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let id = Uuid::new_v4();
    let challenge = format!(
        "Welcome to rrelayer!\n\nClick to sign in with your wallet address to continue.\n\nThis request will not trigger a blockchain transaction or cost any gas fees.\n\nWallet address:\n{}\n\nNonce:\n{}",
        secret_request.address.hex(),
        id
    );

    set_authentication_challenge_cache(&state.cache, &id, &secret_request.address, &challenge)
        .await;

    Ok(Json(GenerateSecretResult { id, challenge, address: secret_request.address }))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthenticateRequest {
    pub id: Uuid,

    #[serde(rename = "signedBy")]
    pub signed_by: EvmAddress,
    pub signature: signers::Signature,
}

/// Authenticates a user using wallet signature verification.
///
/// Verifies that the provided signature was created by signing the cached challenge
/// with the private key corresponding to the claimed address. Upon successful verification,
/// invalidates the challenge and generates a new JWT token pair for the user.
///
/// # Arguments
/// * `state` - The application state containing database and cache connections
/// * `authenticate_request` - The authentication request containing ID, address, and signature
///
/// # Returns
/// * `Ok(Json<TokenPair>)` - A pair of access and refresh tokens for the authenticated user
/// * `Err(StatusCode)` - UNAUTHORIZED for invalid signatures, missing challenges, or non-existent users;
///                       INTERNAL_SERVER_ERROR for database or token generation errors
async fn authenticate(
    State(state): State<Arc<AppState>>,
    Json(authenticate_request): Json<AuthenticateRequest>,
) -> Result<Json<TokenPair>, StatusCode> {
    let cached_result = get_authentication_challenge_cache(
        &state.cache,
        &authenticate_request.id,
        &authenticate_request.signed_by,
    )
    .await
    .ok_or(StatusCode::UNAUTHORIZED)?;

    let address = EvmAddress::new(
        authenticate_request
            .signature
            .recover_address_from_msg(cached_result.as_bytes())
            .map_err(|_| StatusCode::UNAUTHORIZED)?,
    );

    if address != authenticate_request.signed_by {
        return Err(StatusCode::UNAUTHORIZED);
    }

    invalidate_authentication_challenge_cache(
        &state.cache,
        &authenticate_request.id,
        &authenticate_request.signed_by,
    )
    .await;

    let user = state
        .db
        .get_user(&authenticate_request.signed_by)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token_pair = create_auth_tokens(&authenticate_request.signed_by, user.role)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(token_pair))
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    pub token: RefreshToken,
}

/// Refreshes JWT tokens using a valid refresh token.
///
/// Validates the provided refresh token, verifies the user still exists and has the same role,
/// then generates a new pair of access and refresh tokens. This allows users to maintain
/// authentication without re-signing challenges.
///
/// # Arguments
/// * `state` - The application state containing database connections
/// * `refresh_request` - The request containing the refresh token to validate
///
/// # Returns
/// * `Ok(Json<TokenPair>)` - A new pair of access and refresh tokens
/// * `Err(StatusCode)` - UNAUTHORIZED for invalid tokens, non-existent users, or role mismatches;
///                       INTERNAL_SERVER_ERROR for database or token generation errors
async fn refresh_auth_token(
    State(state): State<Arc<AppState>>,
    Json(refresh_request): Json<RefreshRequest>,
) -> Result<Json<TokenPair>, StatusCode> {
    let claims = validate_token(JwtTokenType::Refresh, &refresh_request.token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user = state
        .db
        .get_user(&claims.sub)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if user.role != claims.role {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token_pair = create_auth_tokens(&claims.sub, user.role)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(token_pair))
}

/// Creates the authentication router with all authentication endpoints.
///
/// Sets up the authentication routes including challenge generation, authentication,
/// and token refresh. The refresh endpoint is protected by a refresh JWT token guard.
///
/// # Returns
/// * `Router<Arc<AppState>>` - A configured router with authentication endpoints:
///   - POST /secret/generate - Generates authentication challenges
///   - POST /authenticate - Authenticates users with wallet signatures
///   - POST /refresh - Refreshes JWT tokens (requires valid refresh token)
pub fn create_authentication_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/secret/generate", post(generate_auth_secret))
        .route("/authenticate", post(authenticate))
        .route("/refresh", post(refresh_auth_token).route_layer(from_fn(refresh_jwt_token_guard)))
}
