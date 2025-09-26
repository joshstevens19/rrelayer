use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=all_networks
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=all_networks
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=all_networks
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=all_networks
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=all_networks
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=all_networks
    pub async fn all_networks(&self) -> anyhow::Result<()> {
        info!("Testing network management APIs...");

        let all_networks = self
            .relayer_client
            .sdk
            .network
            .get_all()
            .await
            .context("Failed to get all networks")?;
        info!("All networks: {} found", all_networks.len());

        if all_networks.len() != 1 {
            return Err(anyhow!(
                "Should only bring back 1 network brought back {}",
                all_networks.len()
            ));
        }

        Ok(())
    }
}
