use anyhow::Result;
use dotenv::dotenv;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod client;
mod infrastructure;
mod tests;

use crate::tests::test_runner::TestRunner;
use client::E2ETestConfig;
use infrastructure::{AnvilManager, EmbeddedRRelayerServer};

async fn is_rrelayer_ready() -> bool {
    reqwest::get("http://localhost:3000/health").await.is_ok()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv().ok();

    let test_filter = std::env::var("RRELAYER_TEST_FILTER").ok();

    if let Some(filter) = &test_filter {
        info!("[TEST] Running single test scenario: {}", filter);
    } else {
        info!("[TEST] Running all E2E test scenarios...");
    }

    let config = E2ETestConfig::default();

    info!("Starting Anvil blockchain...");
    let mut anvil_manager = AnvilManager::new(config.anvil_port).await?;
    anvil_manager.start().await?;
    info!("[SUCCESS] Anvil is ready!");

    anvil_manager.mine_blocks_with_transactions(50).await?;

    info!("[START] Starting embedded RRelayer server...");
    let current_dir = std::env::current_dir()?;
    let mut rrelayer_server = EmbeddedRRelayerServer::new(current_dir);
    rrelayer_server.start().await?;

    let mut test_runner = TestRunner::new(config, anvil_manager).await?;

    let test_suite = if let Some(filter) = test_filter {
        test_runner.run_filtered_test(&filter).await
    } else {
        test_runner.run_all_tests().await
    };

    let mut anvil_manager = test_runner.into_anvil_manager();

    rrelayer_server.stop().await?;

    info!("[STOP] Stopping Anvil blockchain...");
    anvil_manager.stop().await?;
    info!("[SUCCESS] Anvil stopped");

    let failed_count = test_suite.failed_count() + test_suite.timeout_count();
    if failed_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
