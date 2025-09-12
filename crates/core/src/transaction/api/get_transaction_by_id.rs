use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    app_state::AppState,
    transaction::{
        get_transaction_by_id,
        types::{Transaction, TransactionId},
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
pub async fn get_transaction_by_id_api(
    State(state): State<Arc<AppState>>,
    Path(id): Path<TransactionId>,
) -> Result<Json<Option<Transaction>>, StatusCode> {
    get_transaction_by_id(&state.cache, &state.db, id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
