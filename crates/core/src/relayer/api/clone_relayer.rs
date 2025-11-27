use std::sync::Arc;

use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use super::create_relayer::CreateRelayerResult;
use crate::relayer::db::CreateRelayerMode;
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    network::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct CloneRelayerRequest {
    #[serde(rename = "newRelayerName")]
    pub new_relayer_name: String,
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
}

/// Clones an existing relayer to a new blockchain network.
pub async fn clone_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(relayer): Json<CloneRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
        .await
        .ok_or(not_found("Could not find provider for the chain id".to_string()))?;

    let relayer = state
        .db
        .create_relayer(
            &relayer.new_relayer_name,
            &relayer.chain_id,
            provider,
            CreateRelayerMode::Clone(relayer_id),
        )
        .await?;

    let current_nonce = provider.get_nonce(&relayer).await?;

    let id = relayer.id;
    let address = relayer.address;

    let network_config =
        state.network_configs.iter().find(|config| config.chain_id == relayer.chain_id);

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
