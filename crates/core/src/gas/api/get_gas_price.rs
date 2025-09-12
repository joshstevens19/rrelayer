use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    app_state::AppState, gas::fee_estimator::base::GasEstimatorResult, network::types::ChainId,
};

/// Retrieves gas price estimates for a specific chain via HTTP API.
///
/// # Arguments
/// * `state` - Application state containing the gas oracle cache
/// * `chain_id` - Chain ID extracted from the URL path
///
/// # Returns
/// * `Ok(Json<GasEstimatorResult>)` - Gas price estimates for all speeds if found
/// * `Err(StatusCode::NOT_FOUND)` - If no gas prices are cached for this chain
pub async fn get_gas_price(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
) -> Result<Json<GasEstimatorResult>, StatusCode> {
    let gas_oracle = state
        .gas_oracle_cache
        .lock()
        .await
        .get_gas_price(&chain_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(gas_oracle))
}
