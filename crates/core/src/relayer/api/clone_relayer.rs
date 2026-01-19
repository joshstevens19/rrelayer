use std::sync::Arc;

use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use super::create_relayer::CreateRelayerResult;
use crate::relayer::db::CreateRelayerMode;
use crate::relayer::{cache::invalidate_relayer_cache, start_relayer_queue, types::RelayerId};
use crate::shared::{not_found, HttpError};
use crate::{app_state::AppState, network::ChainId, provider::find_provider_for_chain_id};

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

    let chain_id = relayer.chain_id;
    let relayer = state
        .db
        .create_relayer(
            &relayer.new_relayer_name,
            &chain_id,
            provider,
            CreateRelayerMode::Clone(relayer_id),
        )
        .await?;

    let id = relayer.id;
    let address = relayer.address;

    invalidate_relayer_cache(&state.cache, &id).await;
    start_relayer_queue(&state, relayer, provider, &chain_id).await?;

    Ok(Json(CreateRelayerResult { id, address }))
}
