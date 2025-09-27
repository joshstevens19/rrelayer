pub mod api;
pub use api::create_basic_auth_routes;

mod basic_auth;
pub use basic_auth::{inject_basic_auth_status, validate_basic_auth};
