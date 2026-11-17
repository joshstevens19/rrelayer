use std::sync::Arc;

use axum::{routing::get, Router};

use crate::app_state::AppState;

mod server_info;

use server_info::get_server_info;

pub fn create_server_routes() -> Router<Arc<AppState>> {
    Router::new().route("/info", get(get_server_info))
}
