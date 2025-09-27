use crate::app_state::NetworkValidateAction;
use crate::rate_limiting::{RateLimitOperation, RateLimiter};
use crate::shared::{not_found, unauthorized, HttpError};
use crate::{
    app_state::AppState,
    transaction::{get_transaction_by_id, types::TransactionId},
};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelTransactionResponse {
    pub success: bool,
    pub cancel_transaction_id: Option<TransactionId>,
}

/// API endpoint to cancel a pending transaction.
/// Creates a new cancel transaction instead of overwriting the original.
pub async fn cancel_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<CancelTransactionResponse>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await?
        .ok_or(not_found("Could not find transaction id".to_string()))?;

    if state.relayer_internal_only.restricted(&transaction.from, &transaction.chain_id) {
        return Err(unauthorized(Some("Relayer can only be used internally".to_string())));
    }

    state.validate_auth_basic_or_api_key(&headers, &transaction.from, &transaction.chain_id)?;

    state.network_permission_validate(
        &transaction.from,
        &transaction.chain_id,
        &transaction.to,
        &transaction.value,
        NetworkValidateAction::Transaction,
    )?;

    let rate_limit_reservation = RateLimiter::check_and_reserve_rate_limit(
        &state,
        &headers,
        &transaction.relayer_id,
        RateLimitOperation::Transaction,
    )
    .await?;

    let cancel_result =
        state.transactions_queues.lock().await.cancel_transaction(&transaction).await?;

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(CancelTransactionResponse {
        success: cancel_result.success,
        cancel_transaction_id: cancel_result.cancel_transaction_id,
    }))
}
