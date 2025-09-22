use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};

use crate::relayer::get_relayer::relayer_exists;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, gas::types::GasPrice, relayer::types::RelayerId};

/// Updates the maximum gas price limit for a relayer.
pub async fn update_relay_max_gas_price(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, cap)): Path<(RelayerId, GasPrice)>,
) -> Result<StatusCode, HttpError> {
    let max_gas_price = if cap.into_u128() > 0 { Some(cap) } else { None };

    let exists = relayer_exists(&state.db, &state.cache, &relayer_id).await?;
    {}
    if exists {
        state.db.update_relayer_max_gas_price(&relayer_id, max_gas_price).await?;

        if let Ok(queue) =
            state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
        {
            queue.lock().await.set_max_gas_price(max_gas_price);
        }

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found("Relayer does not exist".to_string()))
    }
}
