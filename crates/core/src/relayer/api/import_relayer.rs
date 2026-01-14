use std::sync::Arc;

use axum::http::HeaderMap;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::relayer::{
    cache::invalidate_relayer_cache,
    start_relayer_queue,
    types::{Relayer, RelayerId},
};
use crate::shared::{bad_request, conflict, internal_server_error, not_found, HttpError};
use crate::{
    app_state::AppState, network::ChainId, provider::find_provider_for_chain_id,
    shared::common_types::EvmAddress,
};
use chrono::Utc;

#[derive(Debug, Deserialize, Serialize)]
pub struct ImportRelayerRequest {
    /// The name for the relayer
    pub name: String,
    /// The key ID (format depends on the signing provider, e.g., KMS key ARN for AWS KMS)
    #[serde(rename = "keyId")]
    pub key_id: String,
    /// The Ethereum address derived from the key
    pub address: EvmAddress,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportRelayerResult {
    pub id: RelayerId,
    pub address: EvmAddress,
    #[serde(rename = "walletIndex")]
    pub wallet_index: i32,
    #[serde(rename = "keyAlias")]
    pub key_alias: String,
}

/// Imports an existing signing key as a relayer.
///
/// This endpoint works with signing providers that support key import (e.g., AWS KMS).
///
/// It:
/// 1. Validates that the signing provider supports key import
/// 2. Verifies the key exists and is the correct type
/// 3. Assigns the next available wallet_index for the chain
/// 4. Creates the required alias/mapping for the key
/// 5. Inserts the relayer record into the database
/// 6. Starts the relayer's transaction queue
pub async fn import_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    headers: HeaderMap,
    Json(request): Json<ImportRelayerRequest>,
) -> Result<Json<ImportRelayerResult>, HttpError> {
    state.validate_basic_auth_valid(&headers)?;

    if state.private_key_only_networks.contains(&chain_id) {
        return Err(bad_request(
            "Cannot import keys for networks configured with only private_keys.".to_string(),
        ));
    }

    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id)
        .await
        .ok_or(not_found("Could not find provider for the chain id".to_string()))?;

    if !provider.supports_key_import() {
        return Err(bad_request(
            "The signing provider for this network does not support importing existing keys. \
             Key import is supported for: AWS KMS."
                .to_string(),
        ));
    }

    if let Some(existing) =
        state.db.get_relayer_by_address(&request.address, &chain_id).await.map_err(|e| {
            internal_server_error(Some(format!("Failed to check for existing relayer: {}", e)))
        })?
    {
        return Err(conflict(format!(
            "A relayer with address {} already exists for chain {} (id: {})",
            request.address, chain_id, existing.id
        )));
    }

    // Acquire mutex to prevent concurrent relayer creation deadlocks
    let _lock = state.relayer_creation_mutex.lock().await;

    let wallet_index = state.db.get_next_wallet_index(&chain_id).await.map_err(|e| {
        internal_server_error(Some(format!("Failed to get next wallet index: {}", e)))
    })?;

    info!(
        "Importing key {} as relayer '{}' with wallet_index {} on chain {}",
        request.key_id, request.name, wallet_index, chain_id
    );

    // Import the key using the provider's wallet manager
    // This verifies the address matches BEFORE creating any alias (no side effects on failure)
    let import_result = provider
        .import_existing_key(&request.key_id, wallet_index as u32, &request.address)
        .await
        .map_err(|e| bad_request(format!("Failed to import key: {}", e)))?;

    let relayer_id = RelayerId::new();
    let relayer = Relayer {
        id: relayer_id,
        name: request.name.clone(),
        chain_id,
        cloned_from_chain_id: None,
        address: request.address,
        wallet_index,
        max_gas_price: None,
        paused: false,
        eip_1559_enabled: true,
        created_at: Utc::now(),
        is_private_key: false,
    };

    state.db.save_relayer(&relayer).await.map_err(|e| {
        internal_server_error(Some(format!("Failed to insert relayer record: {}", e)))
    })?;

    let relayer_id = relayer.id;
    let relayer = state
        .db
        .get_relayer(&relayer_id)
        .await
        .map_err(|e| internal_server_error(Some(format!("Failed to get relayer: {}", e))))?
        .ok_or_else(|| internal_server_error(Some("Relayer not found after insert".to_string())))?;

    invalidate_relayer_cache(&state.cache, &relayer_id).await;

    // Start the transaction queue for this relayer
    start_relayer_queue(&state, relayer, provider, &chain_id).await?;

    info!(
        "Successfully imported key {} as relayer {} (id: {}, address: {}, wallet_index: {})",
        request.key_id, request.name, relayer_id, request.address, wallet_index
    );

    Ok(Json(ImportRelayerResult {
        id: relayer_id,
        address: request.address,
        wallet_index,
        key_alias: import_result.key_alias,
    }))
}
