use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    app_state::AppState,
    relayer::types::RelayerId,
    shared::common_types::{PagingContext, PagingResult},
    signing::db::read::SignedTypedDataHistory,
};

#[derive(Debug, Deserialize)]
pub struct GetSigningHistoryQuery {
    pub limit: u32,
    pub offset: u32,
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
pub async fn get_signed_typed_data_history(
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
