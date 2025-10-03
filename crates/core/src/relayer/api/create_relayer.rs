use std::sync::Arc;

use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::relayer::db::CreateRelayerMode;
use crate::shared::{bad_request, not_found, HttpError};
use crate::{
    app_state::AppState,
    network::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
    shared::common_types::EvmAddress,
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateRelayerRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateRelayerResult {
    pub id: RelayerId,
    pub address: EvmAddress,
}

/// Creates a new relayer for the specified network.
pub async fn create_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    headers: HeaderMap,
    Json(relayer): Json<CreateRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    // Check if this network is configured with only private keys
    if state.private_key_only_networks.contains(&chain_id) {
        return Err(bad_request("Cannot create new relayers for networks configured with only private_keys. Private keys are imported automatically on startup.".to_string()));
    }

    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id)
        .await
        .ok_or(not_found("Could not find provider for the chain id".to_string()))?;

    // Acquire mutex to prevent concurrent relayer creation deadlocks
    let _lock = state.relayer_creation_mutex.lock().await;

    let relayer = state
        .db
        .create_relayer(&relayer.name, &chain_id, provider, CreateRelayerMode::Create)
        .await?;
    invalidate_relayer_cache(&state.cache, &relayer.id).await;

    let current_nonce = provider.get_nonce(&relayer.wallet_index_type().index()).await?;

    let id = relayer.id;
    let address = relayer.address;

    let network_config = state.network_configs.iter().find(|config| config.chain_id == chain_id);

    let gas_bump_config =
        network_config.map(|config| config.gas_bump_blocks_every.clone()).unwrap_or_default();

    let max_gas_price_multiplier =
        network_config.map(|config| config.max_gas_price_multiplier).unwrap_or(2);

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
                state.safe_proxy_manager.clone(),
                gas_bump_config,
                max_gas_price_multiplier,
            ),
            state.transactions_queues.clone(),
        )
        .await?;

    invalidate_relayer_cache(&state.cache, &id).await;
    Ok(Json(CreateRelayerResult { id, address }))
}
