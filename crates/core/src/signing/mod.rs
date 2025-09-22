mod api;
pub use api::{create_signing_history_routes, SignTextDto, SignTextResult, SignTypedDataResult};
mod db;
pub use db::{SignedTextHistory, SignedTypedDataHistory};
