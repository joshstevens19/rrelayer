use alloy::network::AnyTransactionReceipt;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::shared::{internal_server_error, not_found, HttpError};
use crate::{
    app_state::AppState,
    provider::find_provider_for_chain_id,
    relayer::get_relayer,
    transaction::{
        get_transaction_by_id,
        types::{TransactionHash, TransactionId, TransactionStatus},
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct RelayTransactionStatusResult {
    pub hash: Option<TransactionHash>,
    pub status: TransactionStatus,
    pub receipt: Option<AnyTransactionReceipt>,
}

/// API endpoint to retrieve transaction status and receipt information.
pub async fn get_transaction_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<RelayTransactionStatusResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let transaction = get_transaction_by_id(&state.cache, &state.db, id)
        .await?
        .ok_or(not_found("Transaction id not found".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &transaction.from, &transaction.chain_id)?;

    if matches!(
        transaction.status,
        TransactionStatus::PENDING | TransactionStatus::INMEMPOOL | TransactionStatus::EXPIRED
    ) {
        return Ok(Json(RelayTransactionStatusResult {
            hash: transaction.known_transaction_hash,
            status: transaction.status,
            receipt: None,
        }));
    }

    let relayer = get_relayer(&state.db, &state.cache, &transaction.relayer_id)
        .await?
        .ok_or(not_found("Relayer not found".to_string()))?;

    let hash = match transaction.known_transaction_hash {
        Some(hash) => hash,
        None => {
            return Ok(Json(RelayTransactionStatusResult {
                hash: None,
                status: transaction.status,
                receipt: None,
            }));
        }
    };

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
        .await
        .ok_or(internal_server_error(Some("Evm provider can not be found".to_string())))?;

    let receipt = provider
        .get_receipt(&hash)
        .await
        .map_err(|e| internal_server_error(Some(e.to_string())))?;

    Ok(Json(RelayTransactionStatusResult { hash: Some(hash), status: transaction.status, receipt }))
}
