use std::sync::Arc;

use alloy::rpc::types::TransactionReceipt;
use alloy_eips::eip4844::Blob;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::types::TransactionSpeed;
use crate::{
    app_state::AppState,
    authentication::guards::ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
    provider::find_provider_for_chain_id,
    relayer::{get_relayer, is_relayer_api_key, types::RelayerId},
    shared::common_types::{EvmAddress, PagingContext, PagingQuery, PagingResult},
    transaction::{
        get_transaction_by_id,
        queue_system::TransactionToSend,
        types::{
            Transaction, TransactionData, TransactionHash, TransactionId, TransactionStatus,
            TransactionValue,
        },
    },
};

// TODO! GUARDS
async fn get_transaction_by_id_api(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<Option<Transaction>>, StatusCode> {
    get_transaction_by_id(&state.cache, &state.db, id)
        .await
        .map(|transaction| Json(transaction))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelayTransactionStatusResult {
    pub hash: Option<TransactionHash>,
    pub status: TransactionStatus,
    pub receipt: Option<TransactionReceipt>,
}

// TODO! GUARDS
async fn get_transaction_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<RelayTransactionStatusResult>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, id).await;

    match transaction {
        Ok(Some(transaction)) => {
            if transaction.status == TransactionStatus::Pending ||
                transaction.status == TransactionStatus::Inmempool ||
                transaction.status == TransactionStatus::Expired
            {
                return Ok(Json(RelayTransactionStatusResult {
                    hash: transaction.known_transaction_hash,
                    status: transaction.status,
                    receipt: None,
                }));
            }

            let relayer = get_relayer(&state.db, &state.cache, &transaction.relayer_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            match relayer {
                Some(relayer) => match transaction.known_transaction_hash {
                    Some(hash) => {
                        let provider =
                            find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
                                .await;

                        match provider {
                            Some(provider) => {
                                let receipt = provider
                                    .get_receipt(&hash)
                                    .await
                                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                                Ok(Json(RelayTransactionStatusResult {
                                    hash: Some(hash),
                                    status: transaction.status,
                                    receipt,
                                }))
                            }
                            None => Err(StatusCode::INTERNAL_SERVER_ERROR),
                        }
                    }
                    None => Ok(Json(RelayTransactionStatusResult {
                        hash: None,
                        status: transaction.status,
                        receipt: None,
                    })),
                },
                None => Err(StatusCode::NOT_FOUND),
            }
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RelayTransactionRequest {
    pub to: EvmAddress,
    #[serde(default)]
    pub value: TransactionValue,
    #[serde(default)]
    pub data: TransactionData,
    pub speed: Option<TransactionSpeed>,

    #[serde(default)]
    pub blobs: Option<Vec<Blob>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub id: TransactionId,
    pub hash: TransactionHash,
}

async fn send_transaction(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(transaction): Json<RelayTransactionRequest>,
) -> Result<Json<SendTransactionResult>, StatusCode> {
    // Check if the API key is valid for the relayer
    if !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Extract API key from headers
    let api_key = headers
        .get("x-api-Key")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let transaction_to_send = TransactionToSend::new(
        transaction.to,
        api_key.to_string(),
        transaction.value,
        transaction.data.clone(),
        transaction.speed.clone(),
        transaction.blobs.clone(),
    );

    let result = state
        .transactions_queues
        .lock()
        .await
        .add_transaction(&relayer_id, &transaction_to_send)
        .await;

    match result {
        Ok(transaction) => Ok(Json(SendTransactionResult {
            id: transaction.id,
            hash: transaction.known_transaction_hash.expect("Transaction hash should be set"),
        })),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn replace_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
    Json(replace_with): Json<RelayTransactionRequest>,
) -> Result<Json<bool>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !is_relayer_api_key(&state.db, &state.cache, &transaction.relayer_id, &headers).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state
        .transactions_queues
        .lock()
        .await
        .replace_transaction(&transaction, &replace_with)
        .await;

    match result {
        Ok(status) => Ok(Json(status)),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn cancel_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<bool>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !is_relayer_api_key(&state.db, &state.cache, &transaction.relayer_id, &headers).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = state.transactions_queues.lock().await.cancel_transaction(&transaction).await;

    match result {
        Ok(status) => Ok(Json(status)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

// TODO! add paged caching
async fn get_relayer_transactions(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(paging): Query<PagingQuery>,
    headers: HeaderMap,
    auth_guard: ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
) -> Result<Json<PagingResult<Transaction>>, StatusCode> {
    if auth_guard.is_api_key() &&
        !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let paging_context = PagingContext::new(paging.limit, paging.offset);

    state
        .db
        .get_transactions_for_relayer(&relayer_id, &paging_context)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_transactions_pending_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<usize>, StatusCode> {
    if !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let count =
        state.transactions_queues.lock().await.pending_transactions_count(&relayer_id).await;

    Ok(Json(count))
}
async fn get_transactions_inmempool_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<usize>, StatusCode> {
    if !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let count =
        state.transactions_queues.lock().await.inmempool_transactions_count(&relayer_id).await;

    Ok(Json(count))
}

pub fn create_transactions_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:id", get(get_transaction_by_id_api))
        .route("/status/:id", get(get_transaction_status))
        .route("/relayers/:relayer_id/send", post(send_transaction))
        .route("/replace/:transaction_id", put(replace_transaction))
        .route("/cancel/:transaction_id", put(cancel_transaction))
        .route("/relayers/:relayer_id", get(get_relayer_transactions))
        .route("/relayers/:relayer_id/pending/count", get(get_transactions_pending_count))
        .route("/relayers/:relayer_id/inmempool/count", get(get_transactions_inmempool_count))
}
