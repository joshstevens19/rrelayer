use crate::app_state::NetworkValidateAction;
use crate::common_types::EvmAddress;
use crate::rate_limiting::RateLimiter;
use crate::shared::{bad_request, forbidden, not_found, unauthorized, HttpError};
use crate::signing::db::RecordSignedTypedDataRequest;
use crate::transaction::types::TransactionValue;
use crate::{
    app_state::AppState,
    rate_limiting::RateLimitOperation,
    relayer::{get_relayer_provider_context_by_relayer_id, RelayerId},
};
use alloy::dyn_abi::TypedData;
use alloy::primitives::Signature;
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResult {
    pub signature: Signature,
}

pub async fn sign_typed_data(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(typed_data): Json<TypedData>,
) -> Result<Json<SignTypedDataResult>, HttpError> {
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

    if let Some(chain_id) = typed_data.domain.chain_id {
        let chain_id: u64 = chain_id.to();
        if chain_id != relayer_provider_context.relayer.chain_id.u64() {
            return Err(bad_request("Chain id does not match relayer".to_string()));
        }

        state.network_permission_validate(
            &relayer_provider_context.relayer.address,
            &relayer_provider_context.relayer.chain_id,
            &EvmAddress::new(typed_data.domain.verifying_contract.unwrap_or_default()),
            &TransactionValue::zero(),
            NetworkValidateAction::SigningTypedData,
        )?;

        let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
            &state,
            &headers,
            &relayer_id,
            RateLimitOperation::Signing,
        )
        .await?;

        let signature = relayer_provider_context
            .provider
            .sign_typed_data(&relayer_provider_context.relayer, &typed_data)
            .await?;

        let record_request = RecordSignedTypedDataRequest {
            relayer_id,
            domain_data: serde_json::to_value(&typed_data.domain).unwrap_or_default(),
            message_data: serde_json::to_value(&typed_data.message).unwrap_or_default(),
            primary_type: typed_data.primary_type.clone(),
            signature: signature.into(),
            chain_id: relayer_provider_context.provider.chain_id,
        };

        state.db.record_signed_typed_data(&record_request).await?;

        if let Some(ref webhook_manager) = state.webhook_manager {
            let webhook_manager = webhook_manager.clone();
            let relayer_id_clone = relayer_id;
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
    } else {
        Err(unauthorized(Some(
            "You can not sign typed data with a different chain id to the rrelayer".to_string(),
        )))
    }
}
