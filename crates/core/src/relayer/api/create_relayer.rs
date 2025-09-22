use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    network::types::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
    rrelayer_error,
    shared::common_types::EvmAddress,
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

#[derive(Debug, Deserialize)]
pub struct CreateRelayerRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRelayerResult {
    pub id: RelayerId,
    pub address: EvmAddress,
}

/// Creates a new relayer for the specified blockchain network.
///
/// This endpoint creates a new relayer wallet, initializes it in the database,
/// sets up the transaction queue, and returns the relayer ID and address.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `chain_id` - The blockchain network ID to create the relayer for
/// * `relayer` - Request body containing the relayer name
///
/// # Returns
/// * `Ok(Json<CreateRelayerResult>)` - The new relayer's ID and address
/// * `Err(StatusCode)` - HTTP error code if creation fails
pub async fn create_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    Json(relayer): Json<CreateRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, StatusCode> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Acquire mutex to prevent concurrent relayer creation deadlocks
    let _lock = state.relayer_creation_mutex.lock().await;

    let relayer =
        state.db.create_relayer(&relayer.name, &chain_id, provider, None).await.map_err(|e| {
            rrelayer_error!("{}", e);
            StatusCode::BAD_REQUEST
        })?;

    let current_nonce = provider
        .get_nonce(&relayer.wallet_index)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let id = relayer.id;
    let address = relayer.address;

    state
        .transactions_queues
        .lock()
        .await
        .add_new_relayer(
            TransactionsQueueSetup::new(
                relayer,
                provider.clone(),
                NonceManager::new(current_nonce),
                Default::default(),
                Default::default(),
                Default::default(),
                None, // Safe proxy manager not available for dynamically added relayers
            ),
            state.transactions_queues.clone(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    invalidate_relayer_cache(&state.cache, &id).await;
    Ok(Json(CreateRelayerResult { id, address }))
}
