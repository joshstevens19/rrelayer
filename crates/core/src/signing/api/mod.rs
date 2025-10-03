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

pub use sign_text::{SignTextRequest, SignTextResult};
pub use sign_typed_data::SignTypedDataResult;

pub fn create_signing_routes() -> Router<Arc<AppState>> {
    // All signing routes handle authentication internally via validate_allowed_passed_basic_auth + validate_auth_basic_or_api_key
    Router::new()
        .route("/relayers/:relayer_id/message", post(sign_text::sign_text))
        .route("/relayers/:relayer_id/typed-data", post(sign_typed_data::sign_typed_data))
        .route(
            "/relayers/:relayer_id/text-history",
            get(get_signed_text_history::get_signed_text_history),
        )
        .route(
            "/relayers/:relayer_id/typed-data-history",
            get(get_signed_typed_data_history::get_signed_typed_data_history),
        )
        // @deprecated TODO: remove in a few months
        .route("/:relayer_id/message", post(sign_text::sign_text))
        .route("/:relayer_id/typed-data", post(sign_typed_data::sign_typed_data))
        .route("/:relayer_id/text-history", get(get_signed_text_history::get_signed_text_history))
        .route(
            "/:relayer_id/typed-data-history",
            get(get_signed_typed_data_history::get_signed_typed_data_history),
        )
}
