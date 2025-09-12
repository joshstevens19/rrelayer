use alloy::primitives::PrimitiveSignature;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use google_secretmanager1::client::serde_with::serde_derive::Serialize;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    app_state::AppState,
    relayer::{get_relayer_provider_context_by_relayer_id, types::RelayerId},
    rrelayer_error,
    signing::db::write::RecordSignedTextRequest,
    user_rate_limiting::UserRateLimitError,
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
pub async fn sign_text(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(sign): Json<SignTextDto>,
) -> Result<Json<SignTextResult>, StatusCode> {
    // Apply rate limiting for signing operations (AWS KMS costs)
    if let Some(ref user_rate_limiter) = state.user_rate_limiter {
        let user_identifier = format!("{:?}", relayer_id); // Use relayer ID as signing operations are relayer-specific

        match user_rate_limiter
            .check_rate_limit(&user_identifier, "signing_operations_per_minute", 1)
            .await
        {
            Ok(check) => {
                if !check.allowed {
                    rrelayer_error!(
                        "Signing rate limit exceeded for relayer {}: {}",
                        relayer_id,
                        check.rule_type
                    );
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(UserRateLimitError::LimitExceeded {
                rule_type,
                current,
                limit,
                window_seconds,
            }) => {
                rrelayer_error!(
                    "Signing rate limit exceeded for relayer {}: {}/{} {} in {}s",
                    relayer_id,
                    current,
                    limit,
                    rule_type,
                    window_seconds
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                rrelayer_error!("Rate limiting error for signing: {}", e);
                // Don't block signing for rate limiting errors, just log
            }
        }
    }

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

    let record_request = RecordSignedTextRequest {
        relayer_id: relayer_id.into(),
        message: sign.text.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id.into(),
    };

    if let Err(e) = state.db.record_signed_text(&record_request).await {
        rrelayer_error!("Failed to record signed text: {}", e);
    }

    Ok(Json(SignTextResult { message_signed: sign.text, signature }))
}
