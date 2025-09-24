use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=allowlist_restrictions  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=allowlist_restrictions
    pub async fn allowlist_restrictions(&self) -> anyhow::Result<()> {
        info!("Testing allowlist restrictions...");

        let relayer = self.create_and_fund_relayer("allowlist-restriction-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let allowed_address = self.config.anvil_accounts[1];
        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &allowed_address).await?;

        let allowed_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.1")?.into(),
                TransactionData::empty(),
            )
            .await;

        if allowed_tx_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction to allowlisted address should succeed, but got error: {:?}",
                allowed_tx_result.err()
            ));
        }

        let forbidden_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[2], // Different address
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if forbidden_tx_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction to non-allowlisted address should fail, but succeeded"
            ));
        }

        info!("[SUCCESS] Allowlist restrictions working correctly");
        Ok(())
    }
}
