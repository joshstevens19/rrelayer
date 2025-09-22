use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

pub mod status;

pub fn create_basic_auth_routes() -> Router<Arc<AppState>> {
    Router::new().route("/status", get(status::status))
}
