use anyhow::Result;
use dotenv::dotenv;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod anvil_manager;
mod contract_interactions;
mod embedded_rrelayer;
mod relayer_client;
mod rrelayer_manager;
mod test_config;
mod test_scenarios;

use anvil_manager::AnvilManager;
use embedded_rrelayer::EmbeddedRRelayerServer;
use test_config::E2ETestConfig;
use test_scenarios::TestRunner;

async fn is_rrelayer_ready() -> bool {
    reqwest::get("http://localhost:3000/health").await.is_ok()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv().ok();

    // Check for test filter
    let test_filter = std::env::var("RRELAYER_TEST_FILTER").ok();

    if let Some(filter) = &test_filter {
        info!("🧪 Running single test scenario: {}", filter);
    } else {
        info!("🧪 Running all E2E test scenarios...");
    }

    // Load test configuration
    let config = E2ETestConfig::default();

    // Start Anvil automatically
    info!("🔥 Starting Anvil blockchain...");
    let mut anvil_manager = AnvilManager::new(config.anvil_port).await?;
    anvil_manager.start().await?;
    info!("✅ Anvil is ready!");

    // Mine blocks to establish gas price history for proper gas computation
    anvil_manager.mine_blocks_with_transactions(50).await?;

    // Start embedded RRelayer server
    info!("🚀 Starting embedded RRelayer server...");
    let current_dir = std::env::current_dir()?;
    let mut rrelayer_server = EmbeddedRRelayerServer::new(current_dir);
    rrelayer_server.start().await?;

    // Run the test suite
    let test_runner = TestRunner::new(config).await?;

    let results = if let Some(filter) = test_filter {
        test_runner.run_filtered_test(&filter).await
    } else {
        test_runner.run_all_tests().await
    };

    // Stop the embedded RRelayer server
    rrelayer_server.stop().await?;

    // Stop Anvil
    info!("🛑 Stopping Anvil blockchain...");
    anvil_manager.stop().await?;
    info!("✅ Anvil stopped");

    // Print results
    let mut passed = 0;
    let mut failed = 0;

    for (test_name, result) in results {
        match result {
            Ok(_) => {
                info!("✅ {}: PASSED", test_name);
                passed += 1;
            }
            Err(e) => {
                warn!("❌ {}: FAILED - {}", test_name, e);
                failed += 1;
            }
        }
    }

    info!("📊 Test Results: {} passed, {} failed", passed, failed);

    if failed > 0 {
        std::process::exit(1);
    }

    info!("🎉 All tests passed!");
    Ok(())
}
