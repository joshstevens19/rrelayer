use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    app_state::AppState,
    network::types::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
    rrelayer_error,
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

use super::create_relayer::CreateRelayerResult;

#[derive(Debug, Deserialize)]
pub struct CloneRelayerRequest {
    pub new_relayer_name: String,
    pub chain_id: ChainId,
}

/// Clones an existing relayer to a new blockchain network.
///
/// This endpoint creates a new relayer by copying the wallet from an existing relayer
/// but deploying it to a different chain. The new relayer inherits the same private key
/// but operates on the specified target chain.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `relayer_id` - The ID of the existing relayer to clone from
/// * `relayer` - Request body containing the new relayer name and target chain ID
///
/// # Returns
/// * `Ok(Json<CreateRelayerResult>)` - The cloned relayer's ID and address
/// * `Err(StatusCode)` - HTTP error code if cloning fails
pub async fn clone_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(relayer): Json<CloneRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, StatusCode> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let relayer = state
        .db
        .create_relayer(&relayer.new_relayer_name, &relayer.chain_id, provider, Some(relayer_id))
        .await
        .map_err(|e| {
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
        .add_new_relayer(TransactionsQueueSetup::new(
            relayer,
            provider.clone(),
            NonceManager::new(current_nonce),
            Default::default(),
            Default::default(),
            Default::default(),
            None, // Safe proxy manager not available for dynamically added relayers
        ))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    invalidate_relayer_cache(&state.cache, &id).await;
    Ok(Json(CreateRelayerResult { id, address }))
}
