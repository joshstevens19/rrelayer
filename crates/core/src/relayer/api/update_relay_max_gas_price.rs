use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::{
    app_state::AppState,
    gas::types::GasPrice,
    relayer::{cache::invalidate_relayer_cache, get_relayer, types::RelayerId},
    rrelayer_error,
};

/// Updates the maximum gas price limit for a relayer.
///
/// This endpoint sets or removes the gas price cap for a relayer. When set, the relayer
/// will refuse to process transactions that would require gas prices above this limit.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `cap` - The new gas price cap (0 to remove the cap)
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If update succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
pub async fn update_relay_max_gas_price(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, cap)): Path<(RelayerId, GasPrice)>,
) -> StatusCode {
    let max_gas_price = if cap.into_u128() > 0 { Some(cap) } else { None };
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.update_relayer_max_gas_price(&relayer_id, max_gas_price).await {
            Ok(_) => {
                invalidate_relayer_cache(&state.cache, &relayer_id).await;
                if let Ok(queue) = state
                    .transactions_queues
                    .lock()
                    .await
                    .get_transactions_queue_unsafe(&relayer_id)
                {
                    queue.lock().await.set_max_gas_price(max_gas_price);
                }

                StatusCode::NO_CONTENT
            }
            Err(e) => {
                rrelayer_error!("{}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(e) => {
            rrelayer_error!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
