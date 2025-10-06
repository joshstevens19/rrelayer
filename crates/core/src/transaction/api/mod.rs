use std::sync::Arc;

use axum::{
    routing::{get, post, put},
    Router,
};

use crate::app_state::AppState;

mod cancel_transaction;
pub use cancel_transaction::CancelTransactionResponse;
mod get_relayer_transactions;
mod get_transaction_by_id;
mod get_transaction_status;
pub use get_transaction_status::RelayTransactionStatusResult;
mod get_transactions_inmempool_count;
mod get_transactions_pending_count;
mod replace_transaction;
mod send_transaction;
pub use send_transaction::{RelayTransactionRequest, SendTransactionResult};
mod send_random_transaction;
mod types;
pub use types::TransactionSpeed;

pub fn create_transactions_routes() -> Router<Arc<AppState>> {
    // All transaction routes handle authentication internally via validate_allowed_passed_basic_auth + validate_auth_basic_or_api_key
    Router::new()
        .route("/:id", get(get_transaction_by_id::get_transaction_by_id_api))
        .route("/status/:id", get(get_transaction_status::get_transaction_status))
        .route("/relayers/:relayer_id/send", post(send_transaction::handle_send_transaction))
        .route(
            "/relayers/:chain_id/send_random",
            post(send_random_transaction::send_random_transaction),
        )
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
