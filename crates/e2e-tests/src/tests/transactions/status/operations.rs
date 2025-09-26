use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_status_operations
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_status_operations  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_status_operations
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_status_operations
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_status_operations
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_status_operations
    pub async fn transaction_status_operations(&self) -> anyhow::Result<()> {
        info!("Testing transaction status operations...");

        let relayer = self.create_and_fund_relayer("tx-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-status-op".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send(&relayer.id, &tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;
        info!("Sent transaction for status testing: {}", transaction_id);

        let status_result = self
            .relayer_client
            .sdk
            .transaction
            .get_status(transaction_id)
            .await
            .context("Failed to get transaction status")?;

        if let Some(result) = status_result {
            // this depends on how fast relayer executes the queue
            if result.status != TransactionStatus::PENDING
                && result.status != TransactionStatus::INMEMPOOL
            {
                return Err(anyhow::anyhow!(
                    "Transaction status should be inmempool or pending at this point but it is {}",
                    result.status
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Transaction status not found"));
        }

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        let updated_status = self
            .relayer_client
            .sdk
            .transaction
            .get_status(transaction_id)
            .await
            .context("Failed to get updated transaction status")?;

        if let Some(status) = updated_status {
            if status.status != TransactionStatus::MINED {
                return Err(anyhow::anyhow!("Transaction status should be mined at this point"));
            }
        }

        info!("[SUCCESS] Transaction status operations work correctly");
        Ok(())
    }
}
