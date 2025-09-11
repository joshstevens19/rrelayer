pub mod sign;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use futures::TryFutureExt;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    gas::types::GasPrice,
    network::types::ChainId,
    provider::{chain_enabled, find_provider_for_chain_id},
    relayer::{
        api::sign::create_sign_routes,
        cache::invalidate_relayer_cache,
        get_relayer,
        types::{Relayer, RelayerId},
    },
    rrelayer_error,
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

#[derive(Debug, Deserialize)]
struct CreateRelayerRequest {
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
async fn create_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    Json(relayer): Json<CreateRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, StatusCode> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

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

#[derive(Debug, Deserialize)]
struct CloneRelayerRequest {
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
async fn clone_relayer(
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

#[derive(Debug, Deserialize)]
struct GetRelayersQuery {
    chain_id: Option<ChainId>,
    limit: u32,
    offset: u32,
}

/// Retrieves a paginated list of relayers, optionally filtered by chain ID.
///
/// This endpoint returns a list of all relayers with optional filtering by blockchain network.
/// Results are paginated using limit and offset parameters.
///
/// # Arguments
/// * `state` - Application state containing database connections
/// * `query` - Query parameters including optional chain_id, limit, and offset
///
/// # Returns
/// * `Ok(Json<PagingResult<Relayer>>)` - Paginated list of relayers
/// * `Err(StatusCode)` - HTTP error code if retrieval fails
async fn get_relayers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetRelayersQuery>,
) -> Result<Json<PagingResult<Relayer>>, StatusCode> {
    match query.chain_id {
        Some(chain_id) => {
            if !chain_enabled(&state.evm_providers, &chain_id) {
                return Err(StatusCode::BAD_REQUEST);
            }

            state
                .db
                .get_relayers_for_chain(&chain_id, &PagingContext::new(query.limit, query.offset))
                .await
                .map(Json)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => state
            .db
            .get_relayers(&PagingContext::new(query.limit, query.offset))
            .await
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRelayerResult {
    pub relayer: Relayer,
    #[serde(rename = "providerUrls")]
    pub provider_urls: Vec<String>,
}

/// Retrieves detailed information about a specific relayer.
///
/// This endpoint returns relayer details including its configuration and associated
/// provider URLs.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `auth_guard` - Authentication guard that validates basic auth
/// * `relayer_id` - The unique identifier of the relayer to retrieve
///
/// # Returns
/// * `Ok(Json<GetRelayerResult>)` - Relayer details and provider URLs
/// * `Err(StatusCode)` - HTTP error code if retrieval fails or unauthorized
async fn get_relayer_api(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<GetRelayerResult>, StatusCode> {
    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id).await;
    let provider_urls = provider.map(|p| p.provider_urls.clone()).unwrap_or_default();

    Ok(Json(GetRelayerResult { relayer, provider_urls }))
}

/// Soft deletes a relayer from the system.
///
/// This endpoint marks a relayer as deleted in the database, invalidates its cache,
/// and removes its transaction queue. The relayer data is preserved for audit purposes.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `relayer_id` - The unique identifier of the relayer to delete
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If deletion succeeds
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If deletion fails
async fn delete_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.delete_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            state.transactions_queues.lock().await.delete_queue(&relayer_id).await;
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Pauses transaction processing for a relayer.
///
/// This endpoint stops the relayer from processing new transactions while keeping
/// it in the system. The relayer's queue is paused and the status is updated in the database.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer to pause
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If pause succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn pause_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.pause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_paused(true);
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Resumes transaction processing for a paused relayer.
///
/// This endpoint reactivates a paused relayer, allowing it to resume processing
/// transactions. The relayer's queue is unpaused and the status is updated in the database.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer to unpause
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If unpause succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn unpause_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.unpause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_paused(false);
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Updates the maximum gas price limit for a relayer.
///
/// This endpoint sets or removes the gas price cap for a relayer. When set, the relayer
/// will refuse to process transactions that would require gas prices above this limit.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `cap` - The new gas price cap (None to remove the cap)
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If update succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn update_relay_max_gas_price(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, cap)): Path<(RelayerId, Option<GasPrice>)>,
) -> StatusCode {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.update_relayer_max_gas_price(&relayer_id, cap).await {
            Ok(_) => {
                invalidate_relayer_cache(&state.cache, &relayer_id).await;
                if let Ok(queue) = state
                    .transactions_queues
                    .lock()
                    .await
                    .get_transactions_queue_unsafe(&relayer_id)
                {
                    queue.lock().await.set_max_gas_price(cap);
                }

                StatusCode::NO_CONTENT
            }
            Err(e) => {
                rrelayer_error!("{}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(e) => {
            rrelayer_error!("{}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetAllowlistAddressesQuery {
    limit: u32,
    offset: u32,
}

/// Retrieves the allowlist addresses for a relayer.
///
/// This endpoint returns a paginated list of Ethereum addresses that are allowed
/// to use this relayer for transaction processing when allowlist mode is enabled.
///
/// # Arguments
/// * `state` - Application state containing database connections
/// * `auth_guard` - Authentication guard for access control
/// * `relayer_id` - The unique identifier of the relayer
/// * `query` - Query parameters for pagination (limit and offset)
///
/// # Returns
/// * `Ok(Json<PagingResult<EvmAddress>>)` - Paginated list of allowlisted addresses
/// * `Err(StatusCode::UNAUTHORIZED)` - If authentication fails
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If retrieval fails
async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetAllowlistAddressesQuery>,
) -> Result<Json<PagingResult<EvmAddress>>, StatusCode> {
    get_relayer(&state.db, &state.cache, &relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    state
        .db
        .relayer_get_allowlist_addresses(
            &relayer_id,
            &PagingContext::new(query.limit, query.offset),
        )
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Adds an address to the relayer's allowlist.
///
/// This endpoint adds an Ethereum address to the relayer's allowlist and automatically
/// enables allowlist-only mode for the relayer's transaction queue.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `address` - The Ethereum address to add to the allowlist
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If addition succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn add_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> StatusCode {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.relayer_add_allowlist_address(&relayer_id, &address).await {
            Ok(_) => {
                if let Ok(queue) = state
                    .transactions_queues
                    .lock()
                    .await
                    .get_transactions_queue_unsafe(&relayer_id)
                {
                    queue.lock().await.set_is_allowlisted_only(true);
                }

                StatusCode::NO_CONTENT
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Removes an address from the relayer's allowlist.
///
/// This endpoint removes an Ethereum address from the relayer's allowlist.
/// If no addresses remain in the allowlist, allowlist-only mode is automatically disabled.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `address` - The Ethereum address to remove from the allowlist
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If removal succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::NOT_FOUND` - If relayer doesn't exist
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn delete_allowlist_address(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
) -> StatusCode {
    match state.db.relayer_delete_allowlist_address(&relayer_id, &address).await {
        Ok(_) => match state.db.get_relayer(&relayer_id).await {
            Ok(Some(relayer)) => {
                if !relayer.allowlisted_only {
                    if let Ok(queue) = state
                        .transactions_queues
                        .lock()
                        .await
                        .get_transactions_queue_unsafe(&relayer_id)
                    {
                        queue.lock().await.set_is_allowlisted_only(false);
                    }
                }
                StatusCode::NO_CONTENT
            }
            Ok(None) => StatusCode::NOT_FOUND,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Updates the EIP-1559 transaction status for a relayer.
///
/// This endpoint enables or disables EIP-1559 (London hard fork) transaction support
/// for a relayer. When enabled, the relayer will use type-2 transactions with base fee
/// and priority fee. When disabled, it uses legacy transactions with gas price.
///
/// # Arguments
/// * `state` - Application state containing database and queue connections
/// * `auth_guard` - Authentication guard requiring manager-level permissions
/// * `relayer_id` - The unique identifier of the relayer
/// * `enabled` - Whether to enable EIP-1559 transactions (true) or use legacy (false)
///
/// # Returns
/// * `StatusCode::NO_CONTENT` - If update succeeds
/// * `StatusCode::UNAUTHORIZED` - If authentication fails
/// * `StatusCode::INTERNAL_SERVER_ERROR` - If database operation fails
async fn update_relay_eip1559_status(
    State(state): State<Arc<AppState>>,
    Path((relayer_id, enabled)): Path<(RelayerId, bool)>,
) -> StatusCode {
    match state.db.update_relayer_eip_1559_status(&relayer_id, &enabled).await {
        Ok(_) => {
            if let Ok(queue) =
                state.transactions_queues.lock().await.get_transactions_queue_unsafe(&relayer_id)
            {
                queue.lock().await.set_is_legacy_transactions(!enabled); // Fixed: EIP-1559 enabled = NOT legacy
            }

            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn create_relayer_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:chain_id/new", post(create_relayer))
        .route("/", get(get_relayers))
        .route("/:relayer_id", get(get_relayer_api))
        .route("/:relayer_id", delete(delete_relayer))
        .route("/:relayer_id/pause", put(pause_relayer))
        .route("/:relayer_id/unpause", put(unpause_relayer))
        .route("/:relayer_id/gas/max/:cap", put(update_relay_max_gas_price))
        .route("/:relayer_id/clone", post(clone_relayer))
        .route("/:relayer_id/allowlists", get(get_allowlist_addresses))
        .route("/:relayer_id/allowlists/:address", post(add_allowlist_address))
        .route("/:relayer_id/allowlists/:address", delete(delete_allowlist_address))
        .route("/:relayer_id/gas/eip1559/:enabled", put(update_relay_eip1559_status))
        .nest("/", create_sign_routes())
}
