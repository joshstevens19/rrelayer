use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_send_eth_legacy
    pub async fn transaction_send_eth_legacy(&self) -> anyhow::Result<()> {
        info!("Testing simple eth transfer...");

        let relayer = self.create_and_fund_relayer("eth-transfer-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        self.relayer_client.sdk.relayer.update_eip1559_status(&relayer.id, false).await?;

        let recipient = &self.config.anvil_accounts[1];
        info!("Sending ETH transfer to {}", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id,
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
