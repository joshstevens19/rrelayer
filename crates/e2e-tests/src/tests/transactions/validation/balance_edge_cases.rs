use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_validation_balance_edge_cases
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_validation_balance_edge_cases
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_validation_balance_edge_cases
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_validation_balance_edge_cases
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_validation_balance_edge_cases
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_validation_balance_edge_cases
    pub async fn transaction_validation_balance_edge_cases(&self) -> anyhow::Result<()> {
        info!("Testing balance edge cases...");

        let relayer = self.create_and_fund_relayer("balance-edge-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let excessive_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("100_000")?.into(),
                TransactionData::empty(),
            )
            .await;

        if excessive_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction exceeding balance should fail, but succeeded"
            ));
        }

        let exact_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("10")?.into(),
                TransactionData::empty(),
            )
            .await;

        if exact_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction exceeding balance should fail as not enough gas, but succeeded"
            ));
        }

        let just_under_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("9.98")?.into(),
                TransactionData::empty(),
            )
            .await;

        if just_under_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction has enough balance should be allowed but failed"
            ));
        }

        info!("[SUCCESS] Balance edge cases handled correctly");
        Ok(())
    }
}
