use alloy::dyn_abi::TypedData;
use alloy::primitives::PrimitiveSignature;
use axum::routing::post;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use google_secretmanager1::client::serde_with::serde_derive::Serialize;
use serde::Deserialize;
use std::sync::Arc;
use tracing::error;
use crate::relayer::get_relayer_provider_context_by_relayer_id;
use crate::signing::db::read::{SignedTextHistory, SignedTypedDataHistory};
use crate::signing::db::write::{RecordSignedTextRequest, RecordSignedTypedDataRequest};
use crate::user_rate_limiting::UserRateLimitError;
use crate::{
    app_state::AppState,
    relayer::types::RelayerId,
    rrelayer_error,
    shared::common_types::{PagingContext, PagingResult},
};

#[derive(Debug, Deserialize)]
pub struct SignTextDto {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTextResult {
    #[serde(rename = "messageSigned")]
    pub message_signed: String,
    pub signature: PrimitiveSignature,
}

/// Signs a plain text message using the relayer's private key.
///
/// This endpoint signs a text message using the relayer's wallet, producing a signature
/// that can be used for authentication or verification purposes. The signature follows
/// Ethereum's personal message signing standard.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `sign` - Request body containing the text message to sign
///
/// # Returns
/// * `Ok(Json<SignTextResult>)` - The original message and its signature
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist or no provider found
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If signing operation fails
async fn sign_text(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(sign): Json<SignTextDto>,
) -> Result<Json<SignTextResult>, StatusCode> {
    // Apply rate limiting for signing operations (AWS KMS costs)
    if let Some(ref user_rate_limiter) = state.user_rate_limiter {
        let user_identifier = format!("{:?}", relayer_id); // Use relayer ID as signing operations are relayer-specific

        match user_rate_limiter
            .check_rate_limit(&user_identifier, "signing_operations_per_minute", 1)
            .await
        {
            Ok(check) => {
                if !check.allowed {
                    rrelayer_error!(
                        "Signing rate limit exceeded for relayer {}: {}",
                        relayer_id,
                        check.rule_type
                    );
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(UserRateLimitError::LimitExceeded {
                rule_type,
                current,
                limit,
                window_seconds,
            }) => {
                rrelayer_error!(
                    "Signing rate limit exceeded for relayer {}: {}/{} {} in {}s",
                    relayer_id,
                    current,
                    limit,
                    rule_type,
                    window_seconds
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                rrelayer_error!("Rate limiting error for signing: {}", e);
                // Don't block signing for rate limiting errors, just log
            }
        }
    }

    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let signature = relayer_provider_context
        .provider
        .sign_text(&relayer_provider_context.relayer.wallet_index, &sign.text)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let record_request = RecordSignedTextRequest {
        relayer_id: relayer_id.into(),
        message: sign.text.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id.into(),
    };

    if let Err(e) = state.db.record_signed_text(&record_request).await {
        rrelayer_error!("Failed to record signed text: {}", e);
    }

    Ok(Json(SignTextResult { message_signed: sign.text, signature }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignTypedDataResult {
    pub signature: PrimitiveSignature,
}

/// Signs structured typed data using the relayer's private key (EIP-712).
///
/// This endpoint signs structured typed data according to EIP-712 standard using the
/// relayer's wallet. This is commonly used for signing permit transactions, meta-transactions,
/// and other structured data that requires domain separation.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `typed_data` - The structured typed data to sign following EIP-712 format
///
/// # Returns
/// * `Ok(Json<SignTypedDataResult>)` - The signature of the typed data
/// * `Err(StatusCode::NOT_FOUND)` - If relayer doesn't exist or no provider found
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If signing operation fails
async fn sign_typed_data(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Json(typed_data): Json<TypedData>,
) -> Result<Json<SignTypedDataResult>, StatusCode> {
    if let Some(ref user_rate_limiter) = state.user_rate_limiter {
        let user_identifier = format!("{:?}", relayer_id);

        match user_rate_limiter
            .check_rate_limit(&user_identifier, "signing_operations_per_minute", 1)
            .await
        {
            Ok(check) => {
                if !check.allowed {
                    rrelayer_error!(
                        "Signing rate limit exceeded for relayer {}: {}",
                        relayer_id,
                        check.rule_type
                    );
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
            Err(UserRateLimitError::LimitExceeded {
                rule_type,
                current,
                limit,
                window_seconds,
            }) => {
                rrelayer_error!(
                    "Signing rate limit exceeded for relayer {}: {}/{} {} in {}s",
                    relayer_id,
                    current,
                    limit,
                    rule_type,
                    window_seconds
                );
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Err(e) => {
                rrelayer_error!("Rate limiting error for signing: {}", e);
                // Don't block signing for rate limiting errors, just log
            }
        }
    }

    let relayer_provider_context = get_relayer_provider_context_by_relayer_id(
        &state.db,
        &state.cache,
        &state.evm_providers,
        &relayer_id,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let signature = relayer_provider_context
        .provider
        .sign_typed_data(&relayer_provider_context.relayer.wallet_index, &typed_data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Record the signing activity in the database
    let record_request = RecordSignedTypedDataRequest {
        relayer_id: relayer_id.into(),
        domain_data: serde_json::to_value(&typed_data.domain).unwrap_or_default(),
        message_data: serde_json::to_value(&typed_data.message).unwrap_or_default(),
        primary_type: typed_data.primary_type.clone(),
        signature: signature.into(),
        chain_id: relayer_provider_context.provider.chain_id.into(),
    };

    if let Err(e) = state.db.record_signed_typed_data(&record_request).await {
        rrelayer_error!("Failed to record signed typed data: {}", e);
    }

    Ok(Json(SignTypedDataResult { signature }))
}

#[derive(Debug, Deserialize)]
struct GetSigningHistoryQuery {
    limit: u32,
    offset: u32,
}

/// Retrieves the history of signed text messages with optional filtering.
///
/// This endpoint allows querying signed text message history by relayer ID,
/// signer address, and supports pagination.
///
/// # Query Parameters
/// * `relayer_id` - Optional UUID to filter by specific relayer
/// * `signer_address` - Optional Ethereum address to filter by signer
/// * `limit` - Optional limit for number of results (default: 50)
/// * `offset` - Optional offset for pagination (default: 0)
///
/// # Returns
/// * `Ok(Json<SigningHistoryResponse<SignedTextHistory>>)` - List of signed text messages
/// * `Err(StatusCode::BAD_REQUEST)` - If query parameters are invalid
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If database query fails
async fn get_signed_text_history(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetSigningHistoryQuery>,
) -> Result<Json<PagingResult<SignedTextHistory>>, StatusCode> {
    let paging_context = PagingContext::new(query.limit, query.offset);

    let result = state
        .db
        .get_signed_text_history(&relayer_id, &paging_context)
        .await
        .map_err(|e| {
            error!("{}", e.to_string());
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(result))
}

/// Retrieves the history of signed typed data messages with optional filtering.
///
/// This endpoint allows querying signed EIP-712 typed data history by relayer ID,
/// signer address, and supports pagination.
///
/// # Query Parameters
/// * `relayer_id` - Optional UUID to filter by specific relayer
/// * `signer_address` - Optional Ethereum address to filter by signer
/// * `limit` - Optional limit for number of results (default: 50)
/// * `offset` - Optional offset for pagination (default: 0)
///
/// # Returns
/// * `Ok(Json<SigningHistoryResponse<SignedTypedDataHistory>>)` - List of signed typed data messages
/// * `Err(StatusCode::BAD_REQUEST)` - If query parameters are invalid
/// * `Err(StatusCode::INTERNAL_SERVER_ERROR)` - If database query fails
async fn get_signed_typed_data_history(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    Query(query): Query<GetSigningHistoryQuery>,
) -> Result<Json<PagingResult<SignedTypedDataHistory>>, StatusCode> {
    let paging_context = PagingContext::new(query.limit, query.offset);

    let result = state
        .db
        .get_signed_typed_data_history(&relayer_id, &paging_context)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result))
}

/// Creates and configures the HTTP routes for signing history operations.
///
/// This function sets up the REST API endpoints for querying signing history,
/// including both text message and typed data signing records.
///
/// # Returns
/// * A configured Axum Router with signing history endpoints
pub fn create_signing_history_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:relayer_id/message", post(sign_text))
        .route("/:relayer_id/typed-data", post(sign_typed_data))
        .route("/:relayer_id/text-history", get(get_signed_text_history))
        .route("/:relayer_id/typed-data-history", get(get_signed_typed_data_history))
}
