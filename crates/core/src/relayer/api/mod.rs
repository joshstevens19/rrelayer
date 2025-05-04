pub mod sign;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::from_fn,
    routing::{delete, get, post, put},
    Json, Router,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    authentication::guards::{
        admin_jwt_guard, integrator_or_above_jwt_guard, read_only_or_above_jwt_guard,
        ManagerOrAboveJwtTokenOrApiKeyGuard, ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
    },
    gas::types::GasPrice,
    network::types::ChainId,
    provider::{chain_enabled, find_provider_for_chain_id},
    relayer::{
        api::sign::create_sign_routes,
        cache::invalidate_relayer_cache,
        get_relayer, is_relayer_api_key,
        types::{Relayer, RelayerId},
    },
    rrelayerr_error,
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

fn generate_api_key() -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect()
}

async fn create_relayer(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<ChainId>,
    Json(relayer): Json<CreateRelayerRequest>,
) -> Result<Json<CreateRelayerResult>, StatusCode> {
    let provider = find_provider_for_chain_id(&state.evm_providers, &chain_id).await;

    match provider {
        Some(provider) => {
            let result = state.db.create_relayer(&relayer.name, &chain_id, provider).await;

            match result {
                Ok(relayer) => {
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
                        ))
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    invalidate_relayer_cache(&state.cache, &id).await;
                    Ok(Json(CreateRelayerResult { id, address }))
                }
                Err(e) => {
                    rrelayerr_error!("{}", e);
                    Err(StatusCode::BAD_REQUEST)
                }
            }
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Debug, Deserialize)]
struct GetRelayersQuery {
    chain_id: Option<ChainId>,
    limit: u32,
    offset: u32,
}

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

async fn get_relayer_api(
    State(state): State<Arc<AppState>>,
    ReadOnlyOrAboveJwtTokenOrApiKeyGuard(auth_guard): ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<GetRelayerResult>, StatusCode> {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = get_relayer(&state.db, &state.cache, &relayer_id).await;
    match result {
        Ok(Some(relayer)) => {
            let provider =
                find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id).await;
            let provider_urls = provider.map(|p| p.provider_urls.clone()).unwrap_or_default();
            Ok(Json(GetRelayerResult { relayer, provider_urls }))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn delete_relayer(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> StatusCode {
    match state.db.delete_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn pause_relayer(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match state.db.pause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn unpause_relayer(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match state.db.unpause_relayer(&relayer_id).await {
        Ok(_) => {
            invalidate_relayer_cache(&state.cache, &relayer_id).await;
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn update_relay_max_gas_price(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path((relayer_id, cap)): Path<(RelayerId, Option<GasPrice>)>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.update_relayer_max_gas_price(&relayer_id, cap).await {
            Ok(_) => {
                invalidate_relayer_cache(&state.cache, &relayer_id).await;
                StatusCode::NO_CONTENT
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRelayerApiResult {
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

async fn create_relayer_api_key(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<CreateRelayerApiResult>, StatusCode> {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => {
            let new_api_key = generate_api_key();
            match state.db.create_relayer_api_key(&relayer_id, &new_api_key).await {
                Ok(_) => Ok(Json(CreateRelayerApiResult { api_key: new_api_key })),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize)]
struct GetRelayerApiKeysQuery {
    limit: u32,
    offset: u32,
}

async fn get_relayer_api_keys(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetRelayerApiKeysQuery>,
) -> Result<Json<PagingResult<String>>, StatusCode> {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => state
            .db
            .get_relayer_api_keys(&relayer_id, &PagingContext::new(query.limit, query.offset))
            .await
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct DeleteRelayerApiKeyRequest {
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

async fn delete_relayer_api_key(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(body): Json<DeleteRelayerApiKeyRequest>,
) -> StatusCode {
    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.delete_relayer_api_key(&relayer_id, &body.api_key).await {
            Ok(_) => StatusCode::NO_CONTENT,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Debug, Deserialize)]
struct GetAllowlistAddressesQuery {
    limit: u32,
    offset: u32,
}

async fn get_allowlist_addresses(
    State(state): State<Arc<AppState>>,
    ReadOnlyOrAboveJwtTokenOrApiKeyGuard(auth_guard): ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetAllowlistAddressesQuery>,
    headers: HeaderMap,
) -> Result<Json<PagingResult<EvmAddress>>, StatusCode> {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => state
            .db
            .relayer_get_allowlist_addresses(
                &relayer_id,
                &PagingContext::new(query.limit, query.offset),
            )
            .await
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn add_allowlist_address(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match get_relayer(&state.db, &state.cache, &relayer_id).await {
        Ok(Some(_)) => match state.db.relayer_add_allowlist_address(&relayer_id, &address).await {
            Ok(_) => StatusCode::NO_CONTENT,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        },
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn delete_allowlist_address(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path((relayer_id, address)): Path<(RelayerId, EvmAddress)>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match state.db.relayer_delete_allowlist_address(&relayer_id, &address).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn update_relay_eip1559_status(
    State(state): State<Arc<AppState>>,
    ManagerOrAboveJwtTokenOrApiKeyGuard(auth_guard): ManagerOrAboveJwtTokenOrApiKeyGuard,
    Path((relayer_id, enabled)): Path<(RelayerId, bool)>,
    headers: HeaderMap,
) -> StatusCode {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return StatusCode::UNAUTHORIZED;
    }

    match state.db.update_relayer_eip_1559_status(&relayer_id, &enabled).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn create_relayer_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:chain_id/new", post(create_relayer).route_layer(from_fn(admin_jwt_guard)))
        .route("/", get(get_relayers).route_layer(from_fn(read_only_or_above_jwt_guard)))
        .route("/:relayer_id", get(get_relayer_api))
        .route("/:relayer_id", delete(delete_relayer).route_layer(from_fn(admin_jwt_guard)))
        .route("/:relayer_id/pause", put(pause_relayer))
        .route("/:relayer_id/unpause", put(unpause_relayer))
        .route("/:relayer_id/gas/max/:cap", put(update_relay_max_gas_price))
        .route(
            "/:relayer_id/api-keys",
            post(create_relayer_api_key).route_layer(from_fn(integrator_or_above_jwt_guard)),
        )
        .route(
            "/:relayer_id/api-keys",
            get(get_relayer_api_keys).route_layer(from_fn(integrator_or_above_jwt_guard)),
        )
        .route(
            "/:relayer_id/api-keys/delete",
            post(delete_relayer_api_key).route_layer(from_fn(integrator_or_above_jwt_guard)),
        )
        .route("/:relayer_id/allowlists", get(get_allowlist_addresses))
        .route("/:relayer_id/allowlists/:address", post(add_allowlist_address))
        .route("/:relayer_id/allowlists/:address", delete(delete_allowlist_address))
        .route("/:relayer_id/gas/eip1559/:enabled", put(update_relay_eip1559_status))
        .nest("/", create_sign_routes())
}
