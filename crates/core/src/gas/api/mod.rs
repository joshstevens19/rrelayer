use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware::from_fn,
    routing::get,
    Json, Router,
};

use super::fee_estimator::base::GasEstimatorResult;
use crate::{
    app_state::AppState, authentication::guards::read_only_or_above_jwt_guard,
    network::types::ChainId,
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
async fn get_gas_price(
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

/// Creates and configures the gas-related HTTP routes.
///
/// Sets up routes for gas price retrieval with authentication middleware
/// requiring read-only or above permissions.
///
/// # Returns
/// * `Router<Arc<AppState>>` - Configured router with gas price endpoints
pub fn create_gas_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/price/:chain_id", get(get_gas_price))
        .route_layer(from_fn(read_only_or_above_jwt_guard))
}
