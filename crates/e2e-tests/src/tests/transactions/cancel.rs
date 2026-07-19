use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_cancel
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_cancel  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_cancel
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_cancel
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_cancel
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_cancel
    pub async fn transaction_cancel(&self) -> anyhow::Result<()> {
        info!("Testing transaction cancel operation...");

        let relayer = self.create_and_fund_relayer("tx-cancel-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-original".to_string()),
            blobs: None,
        };

        let send_result = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let next_tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[2],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-after-cancel".to_string()),
            blobs: None,
        };

        let next_send_result = relayer
            .transaction()
            .send(&next_tx_request, None)
            .await
            .context("Failed to send follow-up transaction")?;

        let transaction_id = &send_result.id;

        let cancel_result = relayer
            .transaction()
            .cancel(transaction_id, None)
            .await
            .context("Failed to cancel transaction")?;

        if !cancel_result.success {
            return Err(anyhow::anyhow!("Cancel transaction failed"));
        }

        self.wait_for_transaction_completion(&send_result.id)
            .await
            .context("Cancelled transaction did not complete as a no-op")?;

        let transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        if !transaction.is_noop {
            return Err(anyhow::anyhow!(
                "Expected the transaction to be a no-op {}",
                transaction_id
            ));
        }

        let next_transaction = self
            .wait_for_transaction_completion(&next_send_result.id)
            .await
            .context("Follow-up transaction did not complete after cancelling prior nonce")?;

        if next_transaction.0.is_noop {
            return Err(anyhow::anyhow!("Expected the follow-up transaction to stay intact"));
        }

        info!("[SUCCESS] Transaction {} cancel operation works correctly", transaction_id);

        Ok(())
    }
}
