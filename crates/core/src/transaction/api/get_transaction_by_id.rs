use std::sync::Arc;

use axum::{
    extract::{Path, State},
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
) -> Result<Json<Option<Transaction>>, HttpError> {
    let result = get_transaction_by_id(&state.cache, &state.db, id).await?;

    Ok(Json(result))
}
