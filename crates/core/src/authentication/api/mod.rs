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

async fn generate_auth_secret(
    State(state): State<Arc<AppState>>,
    Json(secret_request): Json<GenerateSecretRequest>,
) -> Result<Json<GenerateSecretResult>, StatusCode> {
    let user = state.db.get_user(&secret_request.address).await;
    match user {
        Ok(_) => {
            let id = Uuid::new_v4();
            let challenge = format!(
                "Welcome to rrelayerr!\n\nClick to sign in with your wallet address to continue.\n\nThis request will not trigger a blockchain transaction or cost any gas fees.\n\nWallet address:\n{}\n\nNonce:\n{}",
                secret_request.address.hex(),
                id
            );

            set_authentication_challenge_cache(
                &state.cache,
                &id,
                &secret_request.address,
                &challenge,
            )
            .await;

            Ok(Json(GenerateSecretResult { id, challenge, address: secret_request.address }))
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthenticateRequest {
    pub id: Uuid,

    #[serde(rename = "signedBy")]
    pub signed_by: EvmAddress,
    pub signature: signers::Signature,
}

async fn authenticate(
    State(state): State<Arc<AppState>>,
    Json(authenticate_request): Json<AuthenticateRequest>,
) -> Result<Json<TokenPair>, StatusCode> {
    if let Some(cached_result) = get_authentication_challenge_cache(
        &state.cache,
        &authenticate_request.id,
        &authenticate_request.signed_by,
    )
    .await
    {
        let address = EvmAddress::new(
            authenticate_request
                .signature
                .recover_address_from_msg(cached_result.as_bytes())
                .unwrap(),
        );
        let is_valid = address == authenticate_request.signed_by;

        if !is_valid {
            return Err(StatusCode::UNAUTHORIZED);
        }

        invalidate_authentication_challenge_cache(
            &state.cache,
            &authenticate_request.id,
            &authenticate_request.signed_by,
        )
        .await;

        let user = state.db.get_user(&authenticate_request.signed_by).await;

        match user {
            Ok(Some(user)) => {
                let token_pair = create_auth_tokens(&authenticate_request.signed_by, user.role);

                match token_pair {
                    Ok(token_pair) => Ok(Json(token_pair)),
                    Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
                }
            }
            Ok(None) | Err(_) => Err(StatusCode::UNAUTHORIZED),
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    pub token: RefreshToken,
}

async fn refresh_auth_token(
    State(state): State<Arc<AppState>>,
    Json(refresh_request): Json<RefreshRequest>,
) -> Result<Json<TokenPair>, StatusCode> {
    let claims = validate_token(JwtTokenType::Refresh, &refresh_request.token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user = state.db.get_user(&claims.sub).await;

    match user {
        Ok(Some(user)) => {
            if user.role != claims.role {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let token_pair = create_auth_tokens(&claims.sub, user.role)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(Json(token_pair))
        }
        Ok(None) | Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

pub fn create_authentication_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/secret/generate", post(generate_auth_secret))
        .route("/authenticate", post(authenticate))
        .route("/refresh", post(refresh_auth_token).route_layer(from_fn(refresh_jwt_token_guard)))
}
