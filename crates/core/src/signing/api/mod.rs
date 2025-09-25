use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::app_state::AppState;

mod get_signed_text_history;
mod get_signed_typed_data_history;
mod sign_text;
mod sign_typed_data;

// Re-export public types for backward compatibility
pub use sign_text::{SignTextDto, SignTextResult};
pub use sign_typed_data::SignTypedDataResult;

/// Creates and configures the HTTP routes for signing history operations.
///
/// This function sets up the REST API endpoints for querying signing history,
/// including both text message and typed data signing records.
///
/// # Returns
/// * A configured Axum Router with signing history endpoints
pub fn create_signing_history_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:relayer_id/message", post(sign_text::sign_text))
        .route("/:relayer_id/typed-data", post(sign_typed_data::sign_typed_data))
        .route("/:relayer_id/text-history", get(get_signed_text_history::get_signed_text_history))
        .route(
            "/:relayer_id/typed-data-history",
            get(get_signed_typed_data_history::get_signed_typed_data_history),
        )
}
