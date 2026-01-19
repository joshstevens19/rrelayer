use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    transaction::types::{Transaction, TransactionHash},
};

/// API endpoint to retrieve a transaction by its blockchain transaction hash.
pub async fn get_transaction_by_tx_hash_api(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<TransactionHash>,
    headers: HeaderMap,
) -> Result<Json<Option<Transaction>>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let result = state.db.get_transaction_by_hash(&tx_hash).await?;
    if let Some(transaction) = &result {
        state.validate_auth_basic_or_api_key(&headers, &transaction.from, &transaction.chain_id)?;
    }

    Ok(Json(result))
}
