use anyhow::{anyhow, Result};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_pause_unpause
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_pause_unpause
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_pause_unpause
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_pause_unpause
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_pause_unpause
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_pause_unpause
    pub async fn relayer_pause_unpause(&self) -> Result<()> {
        info!("Testing relayer pause/unpause...");

        let relayer = self.create_and_fund_relayer("pause-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let normal_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if normal_result.is_err() {
            return Err(anyhow!(
                "Normal transaction should succeed, but got error: {:?}",
                normal_result.err()
            ));
        }

        self.relayer_client.sdk.relayer.pause(&relayer.id).await?;

        let paused_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = paused_config {
            if !config.relayer.paused {
                return Err(anyhow!("Relayer should be paused, but is not"));
            }
        }

        let paused_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if paused_result.is_ok() {
            return Err(anyhow!("Transaction should fail when relayer is paused, but succeeded"));
        }

        self.relayer_client.sdk.relayer.unpause(&relayer.id).await?;

        let unpaused_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = unpaused_config {
            if config.relayer.paused {
                return Err(anyhow!("Relayer should not be paused, but is"));
            }
        }

        let unpaused_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if unpaused_result.is_err() {
            return Err(anyhow!(
                "Transaction should succeed after unpause, but got error: {:?}",
                unpaused_result.err()
            ));
        }

        info!("[SUCCESS] Relayer pause/unpause functionality working correctly");
        Ok(())
    }
}
