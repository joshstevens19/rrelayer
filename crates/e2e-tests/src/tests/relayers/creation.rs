use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context, Result};
use rrelayer_core::network::ChainId;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_creation
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_creation
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_creation
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_creation
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_creation
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_creation
    pub async fn relayer_creation(&self) -> Result<()> {
        info!("Creating test relayer...");

        let created_relayer = self.create_and_fund_relayer("basic-relayer-creation").await?;
        info!("Created relayer: {:?}", created_relayer);

        let relayer = self
            .relayer_client
            .sdk
            .relayer
            .get(&created_relayer.id)
            .await?
            .context("Failed to fetch relayer")?
            .relayer;

        info!("Fetched relayer {:?}", relayer);

        if relayer.paused {
            return Err(anyhow!("Relayer should not be paused"));
        }

        if relayer.name != "basic-relayer-creation" {
            return Err(anyhow!("Relayer should always be the same name"));
        }

        if relayer.address != created_relayer.address {
            return Err(anyhow!("Relayer should be the same address"));
        }

        if relayer.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Relayer should not be the same chain"));
        }

        if relayer.max_gas_price.is_some() {
            return Err(anyhow!("Relayer should have a max gas price"));
        }

        if !relayer.eip_1559_enabled {
            return Err(anyhow!("Relayer should have eip 1559 enabled"));
        }

        Ok(())
    }
}
