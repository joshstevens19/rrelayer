use crate::rate_limiting::RateLimiter;
use crate::shared::{not_found, HttpError};
use crate::signing::db::RecordSignedTypedDataRequest;
use crate::{
    app_state::AppState,
    rate_limiting::RateLimitOperation,
    relayer::{get_relayer_provider_context_by_relayer_id, RelayerId},
};
use alloy::dyn_abi::TypedData;
use alloy::primitives::PrimitiveSignature;
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use google_secretmanager1::client::serde_with::serde_derive::Serialize;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResult {
    pub signature: PrimitiveSignature,
}

/// Signs structured typed data using the relayer's private key (EIP-712).
pub async fn sign_typed_data(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(typed_data): Json<TypedData>,
) -> Result<Json<SignTypedDataResult>, HttpError> {
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
        .sign_typed_data(&relayer_provider_context.relayer.wallet_index, &typed_data)
        .await?;

    let record_request = RecordSignedTypedDataRequest {
        relayer_id: relayer_id.into(),
        domain_data: serde_json::to_value(&typed_data.domain).unwrap_or_default(),
        message_data: serde_json::to_value(&typed_data.message).unwrap_or_default(),
        primary_type: typed_data.primary_type.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id.into(),
    };

    state.db.record_signed_typed_data(&record_request).await?;

    if let Some(ref webhook_manager) = state.webhook_manager {
        let webhook_manager = webhook_manager.clone();
        let relayer_id_clone = relayer_id.clone();
        let chain_id = relayer_provider_context.provider.chain_id;
        let domain_data = serde_json::to_value(&typed_data.domain).unwrap_or_default();
        let message_data = serde_json::to_value(&typed_data.message).unwrap_or_default();
        let primary_type_clone = typed_data.primary_type.clone();
        let signature_clone = signature;

        tokio::spawn(async move {
            let webhook_manager = webhook_manager.lock().await;
            webhook_manager
                .on_typed_data_signed(
                    &relayer_id_clone,
                    chain_id,
                    domain_data,
                    message_data,
                    primary_type_clone,
                    signature_clone,
                )
                .await;
        });
    }

    let result = SignTypedDataResult { signature };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
