use std::sync::Arc;

use axum::{
    routing::{get, post, put},
    Router,
};

use crate::app_state::AppState;

pub mod cancel_transaction;
pub mod get_relayer_transactions;
pub mod get_transaction_by_id;
pub mod get_transaction_status;
pub mod get_transactions_inmempool_count;
pub mod get_transactions_pending_count;
pub mod replace_transaction;
pub mod send_transaction;
pub mod types;

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
        .route("/:id", get(get_transaction_by_id::get_transaction_by_id_api))
        .route("/status/:id", get(get_transaction_status::get_transaction_status))
        .route("/relayers/:relayer_id/send", post(send_transaction::send_transaction))
        .route("/replace/:transaction_id", put(replace_transaction::replace_transaction))
        .route("/cancel/:transaction_id", put(cancel_transaction::cancel_transaction))
        .route("/relayers/:relayer_id", get(get_relayer_transactions::get_relayer_transactions))
        .route(
            "/relayers/:relayer_id/pending/count",
            get(get_transactions_pending_count::get_transactions_pending_count),
        )
        .route(
            "/relayers/:relayer_id/inmempool/count",
            get(get_transactions_inmempool_count::get_transactions_inmempool_count),
        )
}
