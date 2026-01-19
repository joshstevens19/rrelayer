use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::app_state::AppState;

mod clone_relayer;
mod create_relayer;
mod delete_relayer;
mod get_allowlist_addresses;
mod get_relayer;
mod get_relayers;
mod import_relayer;
mod pause_relayer;
mod unpause_relayer;
mod update_relay_eip1559_status;
mod update_relay_max_gas_price;

pub use clone_relayer::CloneRelayerRequest;
pub use create_relayer::{CreateRelayerRequest, CreateRelayerResult};
pub use get_relayer::GetRelayerResult;
pub use get_relayers::GetRelayersQuery;
pub use import_relayer::ImportRelayerResult;

use clone_relayer::clone_relayer;
use create_relayer::create_relayer;
use delete_relayer::delete_relayer;
use get_allowlist_addresses::get_allowlist_addresses;
use get_relayer::get_relayer_api;
use get_relayers::get_relayers;
use import_relayer::import_relayer;
use pause_relayer::pause_relayer;
use unpause_relayer::unpause_relayer;
use update_relay_eip1559_status::update_relay_eip1559_status;
use update_relay_max_gas_price::update_relay_max_gas_price;

pub fn create_relayer_routes() -> Router<Arc<AppState>> {
    // All routes handle authentication internally via validate_allowed_passed_basic_auth + validate_auth_basic_or_api_key
    Router::new()
        .route("/:chain_id/new", post(create_relayer))
        .route("/:chain_id/import", post(import_relayer))
        .route("/", get(get_relayers))
        .route("/:relayer_id", get(get_relayer_api))
        .route("/:relayer_id", delete(delete_relayer))
        .route("/:relayer_id/pause", put(pause_relayer))
        .route("/:relayer_id/unpause", put(unpause_relayer))
        .route("/:relayer_id/gas/max/:cap", put(update_relay_max_gas_price))
        .route("/:relayer_id/clone", post(clone_relayer))
        .route("/:relayer_id/allowlists", get(get_allowlist_addresses))
        .route("/:relayer_id/gas/eip1559/:enabled", put(update_relay_eip1559_status))
}
