use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_get
    pub async fn transaction_get(&self) -> anyhow::Result<()> {
        info!("Testing transaction get operation...");

        let relayer = self.create_and_fund_relayer("tx-get-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-get".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let retrieved_tx = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction(transaction_id)
            .await
            .context("Failed to get transaction")?;

        if let Some(tx) = retrieved_tx {
            self.relayer_client.sent_transaction_compare(tx_request, tx)?;
        } else {
            return Err(anyhow::anyhow!("Transaction not found"));
        }

        info!("[SUCCESS] Transaction get works correctly");

        Ok(())
    }
}
