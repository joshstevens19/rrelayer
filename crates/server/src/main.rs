use startup::start;
use tracing::error;

mod app_state;
mod authentication;
mod gas;
mod logger;
mod middleware;
mod network;
mod postgres;
mod provider;
mod relayer;
mod setup;
mod shared;
mod startup;
mod transaction;
mod user;

#[tokio::main]
async fn main() {
    logger::setup_info_logger();
    let result = start().await;

    if let Err(e) = result {
        error!("Error starting the server: {}", e);
    }
}
