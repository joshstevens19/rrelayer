use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::types::{TransactionData, TransactionId};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_batch
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_batch  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_batch
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_batch
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_batch
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_batch
    pub async fn transaction_batch(&self) -> anyhow::Result<()> {
        info!("Testing batch transactions...");

        for i in 0..3 {
            info!("Mining cleanup block {} before batch test...", i + 1);
            self.mine_and_wait().await?;
        }

        let relayer = self.create_and_fund_relayer("batch-test-relayer").await?;

        info!("Created batch test relayer with ID: {}", relayer.id());

        let mut tx_ids: Vec<TransactionId> = Vec::new();

        for i in 0..3 {
            info!("Preparing to send batch transaction {}/3", i + 1);

            let tx_response = self
                .relayer_client
                .send_transaction(
                    relayer.id(),
                    &self.config.anvil_accounts[4],
                    alloy::primitives::utils::parse_ether("0.01")?.into(),
                    TransactionData::empty(),
                )
                .await?;

            info!("[SUCCESS] Sent batch transaction {}: {:?}", i + 1, tx_response);
            tx_ids.push(tx_response.0.id);

            self.mine_and_wait().await?;
        }

        info!("All {} batch transactions sent, waiting for completion...", tx_ids.len());

        for (i, tx_id) in tx_ids.iter().enumerate() {
            info!("Waiting for batch transaction {} to complete...", i + 1);
            self.wait_for_transaction_completion(tx_id).await?;
            info!("[SUCCESS] Batch transaction {} completed", i + 1);
        }

        info!("[SUCCESS] All batch transactions completed successfully");
        Ok(())
    }
}
