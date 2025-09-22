use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, gas::fee_estimator::GasEstimatorResult, network::ChainId};

/// Retrieves gas price estimates for a specific chain via HTTP API.
pub async fn get_gas_price(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> Result<Json<GasEstimatorResult>, HttpError> {
    let gas_oracle = state
        .gas_oracle_cache
        .lock()
        .await
        .get_gas_price(&chain_id)
        .await
        .ok_or(not_found("gas estimates not found".to_string()))?;

    Ok(Json(gas_oracle))
}
