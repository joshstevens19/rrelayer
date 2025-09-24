use anyhow::{anyhow, Result};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_allowlist_toggle
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_allowlist_toggle
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_allowlist_toggle
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_allowlist_toggle
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_allowlist_toggle
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_allowlist_toggle
    pub async fn relayer_allowlist_toggle(&self) -> Result<()> {
        info!("Testing relayer allowlist toggle...");

        let relayer = self.create_and_fund_relayer("allowlist-toggle-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let initial_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = initial_config {
            if config.relayer.allowlisted_only {
                return Err(anyhow!("Relayer should not be allowlisted only"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        let no_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if no_allowlist_result.is_err() {
            return Err(anyhow!(
                "Transaction should succeed without allowlist, but got error: {:?}",
                no_allowlist_result.err()
            ));
        }

        let allowed_address = &self.config.anvil_accounts[1];
        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &allowed_address).await?;

        let enabled_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        info!("Relayer config after enable attempt: {:?}", enabled_config);
        if let Some(config) = enabled_config {
            if !config.relayer.allowlisted_only {
                return Err(anyhow!("Relayer should be allowlisted only"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        let empty_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[3],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if empty_allowlist_result.is_ok() {
            return Err(anyhow!("Transaction should fail with unknown allowlist, but succeeded"));
        }

        let with_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &allowed_address,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if with_allowlist_result.is_err() {
            return Err(anyhow!(
                "Transaction should succeed with allowlist entry, but got error: {:?}",
                with_allowlist_result.err()
            ));
        }

        self.relayer_client.sdk.relayer.allowlist.delete(&relayer.id, &allowed_address).await?;

        let disabled_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        info!("Final relayer config: {:?}", disabled_config);
        if let Some(config) = disabled_config {
            if config.relayer.allowlisted_only {
                return Err(anyhow!("Relayer should not be allowlisted only"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        info!("[SUCCESS] Allowlist toggle functionality working correctly");
        Ok(())
    }
}
