use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use super::create_relayer::CreateRelayerResult;
use crate::shared::{not_found, HttpError};
use crate::{
    app_state::AppState,
    network::ChainId,
    provider::find_provider_for_chain_id,
    relayer::{cache::invalidate_relayer_cache, types::RelayerId},
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

#[derive(Debug, Deserialize)]
pub struct CloneRelayerRequest {
    pub new_relayer_name: String,
    pub chain_id: ChainId,
}

/// Clones an existing relayer to a new blockchain network.
pub async fn clone_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(relayer): Json<CloneRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, HttpError> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
        .await
        .ok_or(not_found("Could not find provider for the chain id".to_string()))?;

    let relayer = state
        .db
        .create_relayer(&relayer.new_relayer_name, &relayer.chain_id, provider, Some(relayer_id))
        .await?;

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
