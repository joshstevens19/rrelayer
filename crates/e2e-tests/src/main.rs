use anyhow::Result;
use dotenv::dotenv;
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod client;
mod config_manager;
mod infrastructure;
mod tests;
use crate::tests::test_runner::TestRunner;
use crate::tests::test_suite::{TestResult, TestSuite};
use client::E2ETestConfig;
use infrastructure::{AnvilManager, EmbeddedRRelayerServer};

#[derive(Debug, Clone, Copy)]
pub enum SigningProvider {
    Raw,
    AwsSecretManager,
    AwsKms,
    GcpSecretManager,
    Privy,
    Turnkey,
}

impl SigningProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            SigningProvider::Raw => "raw",
            SigningProvider::AwsSecretManager => "aws_secret_manager",
            SigningProvider::AwsKms => "aws_kms",
            SigningProvider::GcpSecretManager => "gcp_secret_manager",
            SigningProvider::Privy => "privy",
            SigningProvider::Turnkey => "turnkey",
        }
    }

    pub fn all_providers() -> Vec<SigningProvider> {
        vec![
            SigningProvider::Raw,
            SigningProvider::AwsSecretManager,
            SigningProvider::AwsKms,
            SigningProvider::GcpSecretManager,
            SigningProvider::Privy,
            // Due to API limits we cant do it
            // SigningProvider::Turnkey,
        ]
    }

    pub fn parse_provider(s: &str) -> Option<SigningProvider> {
        match s {
            "raw" => Some(SigningProvider::Raw),
            "aws_secret_manager" => Some(SigningProvider::AwsSecretManager),
            "aws_kms" => Some(SigningProvider::AwsKms),
            "gcp_secret_manager" => Some(SigningProvider::GcpSecretManager),
            "privy" => Some(SigningProvider::Privy),
            "turnkey" => Some(SigningProvider::Turnkey),
            _ => None,
        }
    }
}

async fn run_single_provider(test_filter: Option<String>) -> Result<(u32, u32, TestSuite)> {
    run_single_provider_with_cleanup(test_filter, false).await
}

async fn run_single_provider_with_cleanup(
    test_filter: Option<String>,
    cleanup_docker: bool,
) -> Result<(u32, u32, TestSuite)> {
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

    // Add timeout to shutdown process to prevent hanging
    let shutdown_future = async {
        if cleanup_docker {
            rrelayer_server.stop_with_docker_cleanup(true).await
        } else {
            rrelayer_server.stop().await
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(15), shutdown_future).await {
        Ok(result) => {
            result?;
            info!("[SUCCESS] RRelayer shutdown completed within timeout");
        }
        Err(_) => {
            error!("[ERROR] RRelayer shutdown timed out after 15 seconds, forcing exit");
            // Force kill any remaining processes as a last resort
            let _ = std::process::Command::new("pkill").args(["-f", "rrelayer"]).output();
        }
    }

    info!("[STOP] Stopping Anvil blockchain...");
    anvil_manager.stop().await?;
    info!("[SUCCESS] Anvil stopped");

    let failed_count = test_suite.failed_count() + test_suite.timeout_count();
    let passed_count = test_suite.passed_count();

    Ok((passed_count as u32, failed_count as u32, test_suite))
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

    let explicit_multi_provider_mode = env::var("RRELAYER_MULTI_PROVIDER").is_ok();
    let specific_providers = env::var("RRELAYER_PROVIDERS").ok();
    let test_filter = env::var("RRELAYER_TEST_FILTER").ok();

    let multi_provider_mode = explicit_multi_provider_mode || specific_providers.is_some();

    config_manager::ensure_default_config()?;

    if multi_provider_mode {
        return run_multi_provider_tests(specific_providers, test_filter).await;
    }

    if let Some(filter) = &test_filter {
        info!("[TEST] Running single test scenario: {}", filter);
    } else {
        info!("[TEST] Running all E2E test scenarios...");
    }

    let (_, failed, _) = run_single_provider(test_filter).await?;

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

async fn run_multi_provider_tests(
    specific_providers: Option<String>,
    test_filter: Option<String>,
) -> Result<()> {
    info!("Multi-Provider E2E Test Mode Activated!");

    let providers = if let Some(provider_list) = specific_providers {
        provider_list.split(',').filter_map(|p| SigningProvider::parse_provider(p.trim())).collect()
    } else {
        SigningProvider::all_providers()
    };

    if providers.is_empty() {
        anyhow::bail!("No valid providers specified. Valid providers: raw, aws_secret_manager, aws_kms, gcp_secret_manager, privy, turnkey");
    }

    info!("Providers to test: {:?}", providers.iter().map(|p| p.as_str()).collect::<Vec<_>>());
    if let Some(test) = &test_filter {
        info!("Test filter: {}", test);
    }

    let config_path = std::path::Path::new("rrelayer.yaml");
    let original_config = std::fs::read_to_string(config_path)?;

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut failed_providers = Vec::new();

    for (i, provider) in providers.iter().enumerate() {
        info!("");
        info!("{}", "=".repeat(60));
        info!("🧪 Testing provider: {} ({}/{})", provider.as_str(), i + 1, providers.len());
        info!("{}", "=".repeat(60));

        if let Err(e) = config_manager::update_yaml_for_provider(&original_config, *provider) {
            error!("Failed to update YAML for {}: {}", provider.as_str(), e);
            failed_providers.push(provider.as_str().to_string());
            continue;
        }

        match run_single_provider_with_cleanup(test_filter.clone(), true).await {
            Ok((passed, failed, test_suite)) => {
                total_passed += passed;
                total_failed += failed;

                // Extract error details from test suite
                let errors: Vec<String> = test_suite
                    .tests
                    .iter()
                    .filter_map(|test| match &test.result {
                        TestResult::Failed(msg) => Some(format!("{}: {}", test.name, msg)),
                        TestResult::Timeout => {
                            Some(format!("{}: Test timed out after 90 seconds", test.name))
                        }
                        _ => None,
                    })
                    .collect();

                save_provider_results(*provider, passed, failed, &errors);

                if failed > 0 {
                    failed_providers.push(provider.as_str().to_string());
                    info!("❌ {} failed ({} passed, {} failed)", provider.as_str(), passed, failed);
                } else {
                    info!("✅ {} passed ({} tests)", provider.as_str(), passed);
                }
            }
            Err(e) => {
                error!("Error running tests for {}: {}", provider.as_str(), e);
                failed_providers.push(provider.as_str().to_string());
                total_failed += 1;

                save_provider_results(*provider, 0, 1, &[format!("Runtime error: {}", e)]);
            }
        }

        // Add a pause between providers to ensure complete isolation
        if i < providers.len() - 1 {
            info!("⏳ Waiting 10 seconds before testing next provider to ensure complete isolation...");
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    }

    std::fs::write(config_path, original_config)?;
    info!("🔄 Original configuration restored");

    create_final_summary_report(&providers, total_passed, total_failed, &failed_providers);

    info!("");
    info!("{}", "=".repeat(60));
    info!("📊 MULTI-PROVIDER TEST SUMMARY");
    info!("{}", "=".repeat(60));
    info!("Total tests passed: {}", total_passed);
    info!("Total tests failed: {}", total_failed);
    info!("Providers tested: {}", providers.len());
    info!("Failed providers: {}", failed_providers.len());
    info!("📁 Results saved in __TEST_RESULTS__/ directory (latest in _LAST_RUN_/)");

    if !failed_providers.is_empty() {
        info!("❌ Failed providers: {}", failed_providers.join(", "));
        std::process::exit(1);
    } else {
        info!("✅ All {} providers passed their tests!", providers.len());
    }

    Ok(())
}

fn save_provider_results(provider: SigningProvider, passed: u32, failed: u32, errors: &[String]) {
    let timestamp = chrono::Utc::now();
    let results_dir = std::path::Path::new("__TEST_RESULTS__");
    let last_run_dir = results_dir.join("_LAST_RUN_");

    // Create both directories
    if let Err(e) = std::fs::create_dir_all(&last_run_dir) {
        error!("Failed to create __TEST_RESULTS__/_LAST_RUN_ directory: {}", e);
        return;
    }

    // Clear any existing results for this provider in _LAST_RUN_
    let provider_pattern = format!("{}_", provider.as_str());
    if let Ok(entries) = std::fs::read_dir(&last_run_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                if filename.starts_with(&provider_pattern) && filename.ends_with(".json") {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        error!("Failed to remove old result file {:?}: {}", entry.path(), e);
                    } else {
                        info!("🗑️ Removed old result file: {:?}", entry.path());
                    }
                }
            }
        }
    }

    let filename = format!("{}_{}.json", provider.as_str(), timestamp.format("%Y%m%d_%H%M%S"));

    // Save to _LAST_RUN_ directory
    let last_run_filepath = last_run_dir.join(&filename);

    // Also save to root __TEST_RESULTS__ for historical record
    let historical_filepath = results_dir.join(&filename);

    let result = serde_json::json!({
        "provider": provider.as_str(),
        "timestamp": timestamp.to_rfc3339(),
        "tests_passed": passed,
        "tests_failed": failed,
        "tests_total": passed + failed,
        "success": failed == 0,
        "errors": errors
    });

    let result_json = serde_json::to_string_pretty(&result).unwrap();

    // Write to _LAST_RUN_
    if let Err(e) = std::fs::write(&last_run_filepath, &result_json) {
        error!("Failed to write test results to _LAST_RUN_ for {}: {}", provider.as_str(), e);
    } else {
        info!("📄 Test results saved to _LAST_RUN_: {:?}", last_run_filepath);
    }

    // Write to historical location
    if let Err(e) = std::fs::write(&historical_filepath, &result_json) {
        error!("Failed to write historical test results for {}: {}", provider.as_str(), e);
    } else {
        info!("📄 Historical test results saved: {:?}", historical_filepath);
    }
}

fn create_final_summary_report(
    providers: &[SigningProvider],
    total_passed: u32,
    total_failed: u32,
    failed_providers: &[String],
) {
    let timestamp = chrono::Utc::now();
    let results_dir = std::path::Path::new("__TEST_RESULTS__");
    let last_run_dir = results_dir.join("_LAST_RUN_");

    // Create both directories
    if let Err(e) = std::fs::create_dir_all(&last_run_dir) {
        error!("Failed to create __TEST_RESULTS__/_LAST_RUN_ directory: {}", e);
        return;
    }

    // Clear any existing summary files in _LAST_RUN_
    if let Ok(entries) = std::fs::read_dir(&last_run_dir) {
        for entry in entries.flatten() {
            if let Some(filename) = entry.file_name().to_str() {
                if filename.starts_with("summary_") && filename.ends_with(".json") {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        error!("Failed to remove old summary file {:?}: {}", entry.path(), e);
                    } else {
                        info!("🗑️ Removed old summary file: {:?}", entry.path());
                    }
                }
            }
        }
    }

    let filename = format!("summary_{}.json", timestamp.format("%Y%m%d_%H%M%S"));

    // Save to _LAST_RUN_ directory
    let last_run_filepath = last_run_dir.join(&filename);

    // Also save to root __TEST_RESULTS__ for historical record
    let historical_filepath = results_dir.join(&filename);

    let summary = serde_json::json!({
        "timestamp": timestamp.to_rfc3339(),
        "multi_provider_test_summary": {
            "total_tests_passed": total_passed,
            "total_tests_failed": total_failed,
            "total_tests": total_passed + total_failed,
            "providers_tested": providers.len(),
            "providers_tested_list": providers.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
            "failed_providers_count": failed_providers.len(),
            "failed_providers": failed_providers,
            "success_rate": if total_passed + total_failed > 0 {
                (total_passed as f64 / (total_passed + total_failed) as f64) * 100.0
            } else {
                0.0
            },
            "overall_success": failed_providers.is_empty()
        }
    });

    let summary_json = serde_json::to_string_pretty(&summary).unwrap();

    // Write to _LAST_RUN_
    if let Err(e) = std::fs::write(&last_run_filepath, &summary_json) {
        error!("Failed to write summary report to _LAST_RUN_: {}", e);
    } else {
        info!("📊 Summary report saved to _LAST_RUN_: {:?}", last_run_filepath);
    }

    // Write to historical location
    if let Err(e) = std::fs::write(&historical_filepath, &summary_json) {
        error!("Failed to write historical summary report: {}", e);
    } else {
        info!("📊 Historical summary report saved: {:?}", historical_filepath);
    }
}
