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
use std::sync::Arc;

/// API endpoint to cancel a pending transaction.
// TODO: should return a new tx hash
pub async fn cancel_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<bool>, HttpError> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await?
        .ok_or(not_found("Could not find transaction id".to_string()))?;

    if state.relayer_internal_only.restricted(&transaction.from, &transaction.chain_id) {
        return Err(unauthorized(Some("Relayer can only be used internally".to_string())));
    }

    state.network_permission_validate(
        &headers,
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

    let status = state.transactions_queues.lock().await.cancel_transaction(&transaction).await?;

    if let Some(reservation) = rate_limit_reservation {
        reservation.commit();
    }

    Ok(Json(status))
}
