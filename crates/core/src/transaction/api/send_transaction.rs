use super::types::TransactionSpeed;
use crate::rate_limiting::RateLimiter;
use crate::relayer::{get_relayer, Relayer};
use crate::shared::utils::convert_blob_strings_to_blobs;
use crate::shared::{internal_server_error, not_found, unauthorized, HttpError};
use crate::{
    app_state::{AppState, NetworkValidateAction},
    rate_limiting::RateLimitOperation,
    relayer::RelayerId,
    shared::common_types::EvmAddress,
    transaction::{
        queue_system::TransactionToSend,
        types::{TransactionData, TransactionHash, TransactionId, TransactionValue},
    },
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayTransactionRequest {
    pub to: EvmAddress,
    #[serde(default)]
    pub value: TransactionValue,
    #[serde(default)]
    pub data: TransactionData,
    pub speed: Option<TransactionSpeed>,
    /// This allows an app to pass their own custom external id in perfect for webhooks
    #[serde(rename = "externalId", skip_serializing_if = "Option::is_none", default)]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blobs: Option<Vec<String>>, // will overflow the stack if you use the Blob type directly
}

impl FromStr for RelayTransactionRequest {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub id: TransactionId,
    pub hash: TransactionHash,
}

/// API endpoint to send a new transaction through a relayer.
pub async fn handle_send_transaction(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(transaction): Json<RelayTransactionRequest>,
) -> Result<Json<SendTransactionResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await?
        .ok_or(not_found("Relayer does not exist".to_string()))?;

    let result = send_transaction(relayer, transaction, &state, &headers).await?;

    Ok(Json(result))
}

pub async fn send_transaction(
    relayer: Relayer,
    transaction: RelayTransactionRequest,
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<SendTransactionResult, HttpError> {
    state.validate_auth_basic_or_api_key(headers, &relayer.address, &relayer.chain_id)?;

    if state.relayer_internal_only.restricted(&relayer.address, &relayer.chain_id) {
        return Err(unauthorized(Some("Relayer can only be used internally".to_string())));
    }

    state.network_permission_validate(
        &relayer.address,
        &relayer.chain_id,
        &transaction.to,
        &transaction.value,
        NetworkValidateAction::Transaction,
    )?;

    // Check if blob transactions are enabled for this network
    if transaction.blobs.is_some() {
        let network_config =
            state.network_configs.iter().find(|n| n.chain_id == relayer.chain_id).ok_or_else(
                || internal_server_error(Some("Network configuration not found".to_string())),
            )?;

        if !network_config.enable_sending_blobs.unwrap_or(false) {
            return Err(internal_server_error(Some(
                "Blob transactions are not enabled for this network".to_string(),
            )));
        }
    }

    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        state,
        headers,
        &relayer.id,
        RateLimitOperation::Transaction,
    )
    .await?;

    let transaction_to_send = TransactionToSend::new(
        transaction.to,
        transaction.value,
        transaction.data.clone(),
        transaction.speed.clone(),
        convert_blob_strings_to_blobs(transaction.blobs)?,
        transaction.external_id,
    );

    let transaction = state
        .transactions_queues
        .lock()
        .await
        .add_transaction(&relayer.id, &transaction_to_send)
        .await?;

    let result = SendTransactionResult {
        id: transaction.id,
        hash: transaction.known_transaction_hash.ok_or(internal_server_error(Some(
            "should always have a known transaction hash".to_string(),
        )))?,
    };

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(result)
}
