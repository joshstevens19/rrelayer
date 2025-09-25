use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};

use crate::shared::HttpError;
use crate::{
    app_state::AppState,
    transaction::{
        get_transaction_by_id,
        types::{Transaction, TransactionId},
    },
};

/// API endpoint to retrieve a transaction by its ID.
pub async fn get_transaction_by_id_api(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
    headers: HeaderMap,
) -> Result<Json<Option<Transaction>>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let result = get_transaction_by_id(&state.cache, &state.db, id).await?;
    if let Some(transaction) = &result {
        state.validate_auth_basic_or_api_key(&headers, &transaction.from, &transaction.chain_id)?;
    }

    Ok(Json(result))
}
