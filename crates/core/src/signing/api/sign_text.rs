use crate::rate_limiting::RateLimiter;
use crate::shared::{not_found, HttpError};
use crate::signing::db::RecordSignedTextRequest;
use crate::{
    app_state::AppState,
    rate_limiting::RateLimitOperation,
    relayer::{get_relayer_provider_context_by_relayer_id, RelayerId},
};
use alloy::primitives::PrimitiveSignature;
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use google_secretmanager1::client::serde_with::serde_derive::Serialize;
use serde::Deserialize;
use std::sync::Arc;

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
pub async fn sign_text(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(sign): Json<SignTextDto>,
) -> Result<Json<SignTextResult>, HttpError> {
    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        &state,
        &headers,
        &relayer_id,
        RateLimitOperation::Signing,
    )
    .await?;

    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await?
    .ok_or(not_found("Relayer does not exist".to_string()))?;

    let signature = relayer_provider_context
        .provider
        .sign_text(&relayer_provider_context.relayer.wallet_index, &sign.text)
        .await?;

    let record_request = RecordSignedTextRequest {
        relayer_id: relayer_id.into(),
        message: sign.text.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id.into(),
    };

    state.db.record_signed_text(&record_request).await?;

    if let Some(ref webhook_manager) = state.webhook_manager {
        let webhook_manager = webhook_manager.clone();
        let relayer_id_clone = relayer_id.clone();
        let chain_id = relayer_provider_context.provider.chain_id;
        let message_clone = sign.text.clone();
        let signature_clone = signature;

        tokio::spawn(async move {
            let webhook_manager = webhook_manager.lock().await;
            webhook_manager
                .on_text_signed(&relayer_id_clone, chain_id, message_clone, signature_clone)
                .await;
        });
    }

    let result = SignTextResult { message_signed: sign.text, signature };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
