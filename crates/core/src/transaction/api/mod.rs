use alloy::network::AnyTransactionReceipt;
use alloy::rpc::types::TransactionReceipt;
use alloy_eips::eip4844::Blob;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

use super::types::TransactionSpeed;
use crate::shared::utils::convert_blob_strings_to_blobs;
use crate::{
    app_state::AppState,
    authentication::guards::ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
    provider::find_provider_for_chain_id,
    user_rate_limiting::{UserRateLimitError, UserDetectionError, UserDetector},
    relayer::{get_relayer, is_relayer_api_key, types::RelayerId},
    rrelayer_error, rrelayer_info,
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

/// API endpoint to retrieve a transaction by its ID.
///
/// # Arguments
/// * `state` - The application state containing cache and database connections
/// * `id` - The transaction ID path parameter
///
/// # Returns
/// * `Ok(Json<Option<Transaction>>)` - The transaction if found, None if not found
/// * `Err(StatusCode)` - INTERNAL_SERVER_ERROR if database query fails
// TODO! GUARDS
async fn get_transaction_by_id_api(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<Option<Transaction>>, StatusCode> {
    get_transaction_by_id(&state.cache, &state.db, id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Response structure for transaction status requests.
///
/// Contains the transaction hash, current status, and receipt if available.
#[derive(Debug, Serialize, Deserialize)]
pub struct RelayTransactionStatusResult {
    pub hash: Option<TransactionHash>,
    pub status: TransactionStatus,
    pub receipt: Option<AnyTransactionReceipt>,
}

/// API endpoint to retrieve transaction status and receipt information.
///
/// Fetches transaction status and optionally retrieves the transaction receipt
/// from the blockchain provider for completed transactions.
///
/// # Arguments
/// * `state` - The application state containing cache, database, and provider connections
/// * `id` - The transaction ID path parameter
///
/// # Returns
/// * `Ok(Json<RelayTransactionStatusResult>)` - Transaction status with hash and receipt
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, INTERNAL_SERVER_ERROR on other failures
// TODO! GUARDS
async fn get_transaction_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<RelayTransactionStatusResult>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Early return for statuses that don't need receipt lookup
    if matches!(
        transaction.status,
        TransactionStatus::Pending | TransactionStatus::Inmempool | TransactionStatus::Expired
    ) {
        return Ok(Json(RelayTransactionStatusResult {
            hash: transaction.known_transaction_hash,
            status: transaction.status,
            receipt: None,
        }));
    }

    let relayer = get_relayer(&state.db, &state.cache, &transaction.relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let hash = match transaction.known_transaction_hash {
        Some(hash) => hash,
        None => {
            return Ok(Json(RelayTransactionStatusResult {
                hash: None,
                status: transaction.status,
                receipt: None,
            }));
        }
    };

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let receipt =
        provider.get_receipt(&hash).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RelayTransactionStatusResult { hash: Some(hash), status: transaction.status, receipt }))
}

/// Request structure for relaying transactions.
///
/// Contains all necessary information to create and relay a blockchain transaction.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayTransactionRequest {
    pub to: EvmAddress,
    #[serde(default)]
    pub value: TransactionValue,
    #[serde(default)]
    pub data: TransactionData,
    pub speed: Option<TransactionSpeed>,
    /// This allows an app to pass their own custom external id in perfect for webhooks
    pub external_id: Option<String>,
    #[serde(default)]
    pub blobs: Option<Vec<String>>, // will overflow the stack if you use the Blob type directly
}

impl FromStr for RelayTransactionRequest {
    type Err = serde_json::Error;

    /// Parses a RelayTransactionRequest from a JSON string.
    ///
    /// # Arguments
    /// * `s` - The JSON string to parse
    ///
    /// # Returns
    /// * `Ok(RelayTransactionRequest)` - The parsed request
    /// * `Err(serde_json::Error)` - If JSON parsing fails
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

/// Response structure for send transaction requests.
///
/// Contains the assigned transaction ID and the blockchain transaction hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct SendTransactionResult {
    pub id: TransactionId,
    pub hash: TransactionHash,
}

/// API endpoint to send a new transaction through a relayer.
///
/// Creates a new transaction and adds it to the transaction queue for processing.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing transaction queues and other services
/// * `relayer_id` - The relayer ID path parameter
/// * `headers` - HTTP headers (for future API key validation)
/// * `transaction` - The transaction request payload
///
/// # Returns
/// * `Ok(Json<SendTransactionResult>)` - The transaction ID and hash if successful
/// * `Err(StatusCode)` - BAD_REQUEST for invalid transactions, INTERNAL_SERVER_ERROR for other failures
async fn send_transaction(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
    Json(transaction): Json<RelayTransactionRequest>,
) -> Result<Json<SendTransactionResult>, StatusCode> {
    // TODO: validate API key
    // if !is_reayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }
    //
    // let api_key = headers
    //     .get("x-api-key")
    //     .and_then(|value| value.to_str().ok())
    //     .ok_or(StatusCode::UNAUTHORIZED)?;

    // Apply rate limiting if enabled
    if let Some(ref user_rate_limiter) = state.user_rate_limiter {
        // Detect user from headers or transaction data (EIP-2771)
        let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        // Get user detection config from app config
        let user_detection_config = state
            .rate_limit_config
            .as_ref()
            .and_then(|config| config.user_detection.clone())
            .unwrap_or_default();
        let user_detector = UserDetector::new(user_detection_config);

        // Detect end user from request
        let transaction_bytes = transaction.data.clone().into_inner();
        let user_context = user_detector
            .detect_user(&headers, Some(&transaction.to), &transaction_bytes, &relayer.address)
            .unwrap_or_else(|_| {
                // Fallback to relayer address if detection fails
                crate::user_rate_limiting::UserContext {
                    user_address: relayer.address,
                    detection_method: crate::user_rate_limiting::UserDetectionMethod::Fallback,
                    transaction_type: crate::user_rate_limiting::TransactionType::Direct,
                }
            });

        let user_identifier = format!("{:?}", user_context.user_address);

        // Check transaction rate limit
        match user_rate_limiter.check_rate_limit(&user_identifier, "transactions_per_minute", 1).await {
            Ok(check) => {
                if !check.allowed {
                    rrelayer_error!(
                        "Rate limit exceeded for user {}: {}",
                        user_identifier,
                        check.rule_type
                    );
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(UserRateLimitError::LimitExceeded { rule_type, current, limit, window_seconds }) => {
                rrelayer_error!(
                    "Rate limit exceeded for user {}: {}/{} {} in {}s",
                    user_identifier,
                    current,
                    limit,
                    rule_type,
                    window_seconds
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                rrelayer_error!("Rate limiting error: {}", e);
                // Don't block transaction for rate limiting errors, just log
            }
        }

        // Record transaction metadata for analytics
        tokio::spawn({
            let user_rate_limiter = user_rate_limiter.clone();
            let relayer_id = relayer_id;
            let user_context = user_context.clone();
            async move {
                let relayer_uuid: uuid::Uuid = relayer_id.into();
                let _ = user_rate_limiter
                    .record_transaction_metadata(
                        None, // Transaction hash not available yet
                        &relayer_uuid,
                        &user_context.user_address,
                        &format!("{:?}", user_context.detection_method).to_lowercase(),
                        &format!("{:?}", user_context.transaction_type).to_lowercase(),
                        None, // Gas usage not known until after execution
                        &["transactions_per_minute".to_string()],
                    )
                    .await;
            }
        });
    }

    let transaction_to_send = TransactionToSend::new(
        transaction.to,
        // api_key.to_string(),
        "bob".to_string(),
        transaction.value,
        transaction.data.clone(),
        transaction.speed.clone(),
        convert_blob_strings_to_blobs(transaction.blobs)?,
        transaction.external_id,
    );

    let transaction = state
        .transactions_queues
        .lock()
        .await
        .add_transaction(&relayer_id, &transaction_to_send)
        .await
        .map_err(|e| {
            rrelayer_error!("{}", e);
            StatusCode::BAD_REQUEST
        })?;

    Ok(Json(SendTransactionResult {
        id: transaction.id,
        hash: transaction.known_transaction_hash.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    }))
}

/// API endpoint to replace an existing pending transaction.
///
/// Replaces a pending transaction with new transaction parameters.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing transaction queues and database
/// * `transaction_id` - The transaction ID to replace
/// * `headers` - HTTP headers (for future API key validation)
/// * `replace_with` - The new transaction parameters
///
/// # Returns
/// * `Ok(Json<bool>)` - True if replacement was successful
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, BAD_REQUEST for invalid replacement
// TODO: should return a new tx hash
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

    // TODO: validate API key
    // if !is_relayer_api_key(&state.db, &state.cache, &transaction.relayer_id, &headers).await {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }

    let status = state
        .transactions_queues
        .lock()
        .await
        .replace_transaction(&transaction, &replace_with)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(Json(status))
}

/// API endpoint to cancel a pending transaction.
///
/// Cancels a pending transaction by sending a replacement with higher gas price.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing transaction queues and database
/// * `transaction_id` - The transaction ID to cancel
/// * `headers` - HTTP headers (for future API key validation)
///
/// # Returns
/// * `Ok(Json<bool>)` - True if cancellation was successful
/// * `Err(StatusCode)` - NOT_FOUND if transaction doesn't exist, INTERNAL_SERVER_ERROR for other failures
// TODO: should return a new tx hash
async fn cancel_transaction(
    State(state): State<Arc<AppState>>,
    Path(transaction_id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<bool>, StatusCode> {
    let transaction = get_transaction_by_id(&state.cache, &state.db, transaction_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // TODO: validate API key
    // if !is_relayer_api_key(&state.db, &state.cache, &transaction.relayer_id, &headers).await {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }

    let status = state
        .transactions_queues
        .lock()
        .await
        .cancel_transaction(&transaction)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(status))
}

/// API endpoint to retrieve all transactions for a specific relayer.
///
/// Returns a paginated list of transactions associated with the given relayer.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing the database connection
/// * `relayer_id` - The relayer ID path parameter
/// * `paging` - Query parameters for pagination (limit, offset)
/// * `headers` - HTTP headers (for future API key validation)
/// * `auth_guard` - Authentication guard for access control
///
/// # Returns
/// * `Ok(Json<PagingResult<Transaction>>)` - Paginated list of transactions
/// * `Err(StatusCode)` - INTERNAL_SERVER_ERROR if database query fails
// TODO! add paged caching
async fn get_relayer_transactions(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(paging): Query<PagingQuery>,
    headers: HeaderMap,
    auth_guard: ReadOnlyOrAboveJwtTokenOrApiKeyGuard,
) -> Result<Json<PagingResult<Transaction>>, StatusCode> {
    // TODO: validate API key
    // if auth_guard.is_api_key()
    //     && !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await
    // {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }

    let paging_context = PagingContext::new(paging.limit, paging.offset);

    state
        .db
        .get_transactions_for_relayer(&relayer_id, &paging_context)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// API endpoint to get the count of pending transactions for a relayer.
///
/// Returns the number of transactions currently pending in the queue for the given relayer.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing transaction queues
/// * `relayer_id` - The relayer ID path parameter
/// * `headers` - HTTP headers (for future API key validation)
///
/// # Returns
/// * `Ok(Json<usize>)` - The count of pending transactions
async fn get_transactions_pending_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<usize>, StatusCode> {
    // TODO: validate API key
    // if !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }

    let count =
        state.transactions_queues.lock().await.pending_transactions_count(&relayer_id).await;

    Ok(Json(count))
}

/// API endpoint to get the count of in-mempool transactions for a relayer.
///
/// Returns the number of transactions currently in the mempool for the given relayer.
/// Currently API key validation is disabled (TODO).
///
/// # Arguments
/// * `state` - The application state containing transaction queues
/// * `relayer_id` - The relayer ID path parameter
/// * `headers` - HTTP headers (for future API key validation)
///
/// # Returns
/// * `Ok(Json<usize>)` - The count of in-mempool transactions
async fn get_transactions_inmempool_count(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<usize>, StatusCode> {
    // TODO: validate API key
    // if !is_relayer_api_key(&state.db, &state.cache, &relayer_id, &headers).await {
    //     return Err(StatusCode::UNAUTHORIZED);
    // }

    let count =
        state.transactions_queues.lock().await.inmempool_transactions_count(&relayer_id).await;

    Ok(Json(count))
}

/// Creates and configures the transaction API routes.
///
/// Sets up all HTTP routes for transaction-related operations including:
/// - GET /:id - Get transaction by ID
/// - GET /status/:id - Get transaction status and receipt
/// - POST /relayers/:relayer_id/send - Send new transaction
/// - PUT /replace/:transaction_id - Replace pending transaction
/// - PUT /cancel/:transaction_id - Cancel pending transaction
/// - GET /relayers/:relayer_id - Get relayer transactions (paginated)
/// - GET /relayers/:relayer_id/pending/count - Get pending transaction count
/// - GET /relayers/:relayer_id/inmempool/count - Get in-mempool transaction count
///
/// # Returns
/// * `Router<Arc<AppState>>` - Configured router with all transaction routes
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
