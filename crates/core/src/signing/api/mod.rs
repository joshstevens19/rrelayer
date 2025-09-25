use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::app_state::AppState;

mod get_signed_text_history;
mod get_signed_typed_data_history;
mod sign_text;
mod sign_typed_data;

pub use sign_text::{SignTextDto, SignTextResult};
pub use sign_typed_data::SignTypedDataResult;

pub fn create_signing_routes() -> Router<Arc<AppState>> {
    // All signing routes handle authentication internally via validate_allowed_passed_basic_auth + validate_auth_basic_or_api_key
    Router::new()
        .route("/:relayer_id/message", post(sign_text::sign_text))
        .route("/:relayer_id/typed-data", post(sign_typed_data::sign_typed_data))
        .route("/:relayer_id/text-history", get(get_signed_text_history::get_signed_text_history))
        .route(
            "/:relayer_id/typed-data-history",
            get(get_signed_typed_data_history::get_signed_typed_data_history),
        )
}
