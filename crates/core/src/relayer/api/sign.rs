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

pub fn create_sign_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:relayer_id/sign/message", post(sign_text))
        .route("/:relayer_id/sign/typed-data", post(sign_typed_data))
    // .route_layer(from_fn(relayer_api_key_guard))
}
