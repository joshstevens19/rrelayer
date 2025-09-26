use std::sync::Arc;

use crate::gas::GasEstimatorResult;
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, network::ChainId};
use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};

/// Retrieves gas price estimates for a specific chain via HTTP API.
pub async fn get_gas_price(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    headers: HeaderMap,
) -> Result<Json<GasEstimatorResult>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    let gas_oracle = state
        .gas_oracle_cache
        .lock()
        .await
        .get_gas_price(&chain_id)
        .await
        .ok_or(not_found("gas estimates not found".to_string()))?;

    Ok(Json(gas_oracle))
}
