use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_get
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_get  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_get
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_get
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_get
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_get
    pub async fn transaction_get(&self) -> anyhow::Result<()> {
        info!("Testing transaction get operation...");

        let relayer = self.create_and_fund_relayer("tx-get-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-get".to_string()),
            blobs: None,
        };

        let send_result = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let retrieved_tx =
            relayer.transaction().get(transaction_id).await.context("Failed to get transaction")?;

        if let Some(tx) = retrieved_tx {
            self.relayer_client.sent_transaction_compare(tx_request, tx)?;
        } else {
            return Err(anyhow::anyhow!("Transaction not found"));
        }

        info!("[SUCCESS] Transaction get works correctly");

        Ok(())
    }
}
