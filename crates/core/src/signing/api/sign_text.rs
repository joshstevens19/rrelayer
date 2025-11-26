use crate::common_types::EvmAddress;
use crate::rate_limiting::RateLimiter;
use crate::shared::{forbidden, not_found, HttpError};
use crate::signing::db::RecordSignedTextRequest;
use crate::{
    app_state::AppState,
    rate_limiting::RateLimitOperation,
    relayer::{get_relayer_provider_context_by_relayer_id, RelayerId},
};
use alloy::primitives::Signature;
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
pub struct SignTextRequest {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTextResult {
    #[serde(rename = "messageSigned")]
    pub message_signed: String,
    pub signature: Signature,
    #[serde(rename = "signedBy")]
    pub signed_by: EvmAddress,
}

/// Signs a plain text message using the relayer's private key.
pub async fn sign_text(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(sign): Json<SignTextRequest>,
) -> Result<Json<SignTextResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await?
    .ok_or(not_found("Relayer does not exist".to_string()))?;

    state.validate_auth_basic_or_api_key(
        &headers,
        &relayer_provider_context.relayer.address,
        &relayer_provider_context.relayer.chain_id,
    )?;

    if relayer_provider_context.relayer.paused {
        return Err(forbidden("Relayer is paused".to_string()));
    }

    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        &state,
        &headers,
        &relayer_id,
        RateLimitOperation::Signing,
    )
    .await?;

    state.restricted_personal_signing(
        &relayer_provider_context.relayer.address,
        &relayer_provider_context.relayer.chain_id,
    )?;

    let signature = relayer_provider_context
        .provider
        .sign_text(&relayer_provider_context.relayer, &sign.text)
        .await?;

    let record_request = RecordSignedTextRequest {
        relayer_id,
        message: sign.text.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id,
    };

    state.db.record_signed_text(&record_request).await?;

    if let Some(ref webhook_manager) = state.webhook_manager {
        let webhook_manager = webhook_manager.clone();
        let relayer_id_clone = relayer_id;
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

    let result = SignTextResult {
        message_signed: sign.text,
        signature,
        signed_by: relayer_provider_context.relayer.address,
    };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
