use alloy::network::AnyTransactionReceipt;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    app_state::AppState,
    provider::find_provider_for_chain_id,
    relayer::get_relayer,
    transaction::{
        get_transaction_by_id,
        types::{TransactionHash, TransactionId, TransactionStatus},
    },
};

/// Response structure for transaction status requests.
///
/// Contains the transaction hash, current status, and receipt if available.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelayTransactionStatusResult {
    pub hash: Option<TransactionHash>,
    pub status: TransactionStatus,
    pub receipt: Option<AnyTransactionReceipt>,
}

/// API endpoint to retrieve transaction status and receipt information.
///
/// Fetches transaction status and optionally retrieves the transaction receipt
/// from the blockchain provider for completed transactions.
///
/// # Arguments
/// * `state` - The application state containing cache, database, and provider connections
/// * `id` - The transaction ID path parameter
///
/// # Returns
/// * `Ok(Json<RelayTransactionStatusResult>)` - Transaction status with hash and receipt
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, INTERNAL_SERVER_ERROR on other failures
pub async fn get_transaction_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<RelayTransactionStatusResult>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if matches!(
        transaction.status,
        TransactionStatus::Pending | TransactionStatus::Inmempool | TransactionStatus::Expired
    ) {
        return Ok(Json(RelayTransactionStatusResult {
            hash: transaction.known_transaction_hash,
            status: transaction.status,
            receipt: None,
        }));
    }

    let relayer = get_relayer(&state.db, &state.cache, &transaction.relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

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
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let receipt =
        provider.get_receipt(&hash).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RelayTransactionStatusResult { hash: Some(hash), status: transaction.status, receipt }))
}
