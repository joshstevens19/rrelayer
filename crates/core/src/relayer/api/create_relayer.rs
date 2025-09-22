use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    network::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
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

/// Creates a new relayer for the specified network.
pub async fn create_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    Json(relayer): Json<CreateRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, HttpError> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id)
        .await
        .ok_or(not_found("Could not find provider for the chain id".to_string()))?;

    // Acquire mutex to prevent concurrent relayer creation deadlocks
    let _lock = state.relayer_creation_mutex.lock().await;

    let relayer = state.db.create_relayer(&relayer.name, &chain_id, provider, None).await?;
    invalidate_relayer_cache(&state.cache, &relayer.id).await;

    let current_nonce = provider.get_nonce(&relayer.wallet_index).await?;

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
                None,
            ),
            state.transactions_queues.clone(),
        )
        .await?;

    invalidate_relayer_cache(&state.cache, &id).await;
    Ok(Json(CreateRelayerResult { id, address }))
}
