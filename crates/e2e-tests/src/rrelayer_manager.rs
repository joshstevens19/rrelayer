use anyhow::{Context, Result};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
use tracing::info;

use rrelayer_core::start;

pub struct RRelayerManager {
    temp_dir: TempDir,
    config_path: PathBuf,
}

impl RRelayerManager {
    pub async fn new(anvil_port: u16) -> Result<Self> {
        let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
        let config_path = temp_dir.path().join("rrelayer.yaml");

        // Create test configuration
        let test_config = Self::create_test_config(anvil_port);
        tokio::fs::write(&config_path, test_config).await.context("Failed to write test config")?;

        info!("Created test config at: {:?}", config_path);

        Ok(Self { temp_dir, config_path })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting RRelayer with test configuration...");

        let project_path = self.temp_dir.path().to_path_buf();

        // Start RRelayer in a separate task so it doesn't block
        let _config_path = self.config_path.clone();
        tokio::spawn(async move {
            if let Err(e) = start(&project_path).await {
                tracing::error!("RRelayer startup failed: {}", e);
            }
        });

        // Wait for RRelayer to be ready
        self.wait_for_ready().await?;

        Ok(())
    }

    async fn wait_for_ready(&self) -> Result<()> {
        info!("Waiting for RRelayer to be ready...");

        let client = reqwest::Client::new();

        for attempt in 1..=30 {
            // Wait up to 60 seconds
            match client.get("http://localhost:3000/health").send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("RRelayer is ready!");
                        return Ok(());
                    }
                }
                Err(e) => {
                    info!("Attempt {}/30 - RRelayer not ready: {}", attempt, e);
                }
            }

            sleep(Duration::from_secs(2)).await;
        }

        anyhow::bail!("RRelayer failed to become ready after 60 seconds");
    }

    fn create_test_config(anvil_port: u16) -> String {
        // Create a minimal config that matches the expected structure
        format!(
            r#"# RRelayer E2E Test Configuration
name: "e2e-test"
description: "E2E Testing Configuration"

# Test signing key - using raw mnemonic for simplicity
signing_key:
  raw:
    mnemonic: "test test test test test test test test test test test junk"

networks:
  - name: "anvil-testnet" 
    provider_urls:
      - "http://127.0.0.1:{}"

api_config:
  port: 3000

# Optional features - leaving empty/null for testing
# webhooks: 
# safe_proxy: 
# user_rate_limits:
"#,
            anvil_port
        )
    }

    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn get_project_path(&self) -> PathBuf {
        self.temp_dir.path().to_path_buf()
    }
}
