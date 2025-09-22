use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::app_state::AppState;

mod add_allowlist_address;
mod clone_relayer;
mod create_relayer;
mod delete_allowlist_address;
mod delete_relayer;
mod get_allowlist_addresses;
mod get_relayer_api;
mod get_relayers;
mod pause_relayer;
mod unpause_relayer;
mod update_relay_eip1559_status;
mod update_relay_max_gas_price;

// Re-export public types from endpoint modules
pub use clone_relayer::CloneRelayerRequest;
pub use create_relayer::{CreateRelayerRequest, CreateRelayerResult};
pub use get_allowlist_addresses::GetAllowlistAddressesQuery;
pub use get_relayer_api::GetRelayerResult;
pub use get_relayers::GetRelayersQuery;

// Import handler functions
use add_allowlist_address::add_allowlist_address;
use clone_relayer::clone_relayer;
use create_relayer::create_relayer;
use delete_allowlist_address::delete_allowlist_address;
use delete_relayer::delete_relayer;
use get_allowlist_addresses::get_allowlist_addresses;
use get_relayer_api::get_relayer_api;
use get_relayers::get_relayers;
use pause_relayer::pause_relayer;
use unpause_relayer::unpause_relayer;
use update_relay_eip1559_status::update_relay_eip1559_status;
use update_relay_max_gas_price::update_relay_max_gas_price;

pub fn create_relayer_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:chain_id/new", post(create_relayer))
        .route("/", get(get_relayers))
        .route("/:relayer_id", get(get_relayer_api))
        .route("/:relayer_id", delete(delete_relayer))
        .route("/:relayer_id/pause", put(pause_relayer))
        .route("/:relayer_id/unpause", put(unpause_relayer))
        .route("/:relayer_id/gas/max/:cap", put(update_relay_max_gas_price))
        .route("/:relayer_id/clone", post(clone_relayer))
        .route("/:relayer_id/allowlists", get(get_allowlist_addresses))
        .route("/:relayer_id/allowlists/:address", post(add_allowlist_address))
        .route("/:relayer_id/allowlists/:address", delete(delete_allowlist_address))
        .route("/:relayer_id/gas/eip1559/:enabled", put(update_relay_eip1559_status))
}
