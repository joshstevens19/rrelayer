use super::send_transaction::RelayTransactionRequest;
use crate::app_state::NetworkValidateAction;
use crate::rate_limiting::{RateLimitOperation, RateLimiter};
use crate::shared::{not_found, unauthorized, HttpError};
use crate::transaction::queue_system::ReplaceTransactionResult;
use crate::{
    app_state::AppState,
    transaction::{get_transaction_by_id, types::TransactionId},
};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;

/// API endpoint to replace an existing pending transaction.
/// Returns the new transaction ID and hash for tracking the replacement.
pub async fn replace_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
    Json(replace_with): Json<RelayTransactionRequest>,
) -> Result<Json<ReplaceTransactionResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await?
        .ok_or(not_found("Could not find transaction id".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &transaction.from, &transaction.chain_id)?;

    if state.relayer_internal_only.restricted(&transaction.from, &transaction.chain_id) {
        return Err(unauthorized(Some("Relayer can only be used internally".to_string())));
    }

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

    let result = state
        .transactions_queues
        .lock()
        .await
        .replace_transaction(&transaction, &replace_with)
        .await?;

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(result))
}
