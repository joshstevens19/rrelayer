use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_send_eth
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_send_eth
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_send_eth
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_send_eth
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_send_eth
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_send_eth
    pub async fn transaction_send_eth(&self) -> anyhow::Result<()> {
        info!("Testing simple eth transfer...");

        let relayer = self.create_and_fund_relayer("eth-transfer-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let recipient = &self.config.anvil_accounts[1];
        info!("Sending ETH transfer to {}", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id(),
                recipient,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer")?;

        info!("ETH transfer sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        Ok(())
    }
}
