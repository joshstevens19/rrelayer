use std::sync::Arc;

use alloy::{dyn_abi::TypedData, primitives::PrimitiveSignature};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    relayer::{get_relayer_provider_context_by_relayer_id, types::RelayerId},
};

#[derive(Debug, Deserialize)]
pub struct SignTextDto {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTextResult {
    #[serde(rename = "messageSigned")]
    pub message_signed: String,
    pub signature: PrimitiveSignature,
}

/// Signs a plain text message using the relayer's private key.
///
/// This endpoint signs a text message using the relayer's wallet, producing a signature
/// that can be used for authentication or verification purposes. The signature follows
/// Ethereum's personal message signing standard.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `sign` - Request body containing the text message to sign
///
/// # Returns
/// * `Ok(Json<SignTextResult>)` - The original message and its signature
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist or no provider found
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If signing operation fails
// TODO: handle guard
async fn sign_text(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(sign): Json<SignTextDto>,
) -> Result<Json<SignTextResult>, StatusCode> {
    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let signature = relayer_provider_context
        .provider
        .sign_text(&relayer_provider_context.relayer.wallet_index, &sign.text)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SignTextResult { message_signed: sign.text, signature }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResult {
    pub signature: PrimitiveSignature,
}

/// Signs structured typed data using the relayer's private key (EIP-712).
///
/// This endpoint signs structured typed data according to EIP-712 standard using the
/// relayer's wallet. This is commonly used for signing permit transactions, meta-transactions,
/// and other structured data that requires domain separation.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `typed_data` - The structured typed data to sign following EIP-712 format
///
/// # Returns
/// * `Ok(Json<SignTypedDataResult>)` - The signature of the typed data
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist or no provider found
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If signing operation fails
// TODO: handle guard
async fn sign_typed_data(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(typed_data): Json<TypedData>,
) -> Result<Json<SignTypedDataResult>, StatusCode> {
    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let signature = relayer_provider_context
        .provider
        .sign_typed_data(&relayer_provider_context.relayer.wallet_index, &typed_data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SignTypedDataResult { signature }))
}

/// Creates and configures the HTTP routes for relayer signing operations.
///
/// This function sets up the REST API endpoints for signing operations using relayers,
/// including text message signing and EIP-712 typed data signing.
///
/// # Returns
/// * A configured Axum Router with signing endpoints
pub fn create_sign_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:relayer_id/sign/message", post(sign_text))
        .route("/:relayer_id/sign/typed-data", post(sign_typed_data))
    // .route_layer(from_fn(relayer_api_key_guard))
}
