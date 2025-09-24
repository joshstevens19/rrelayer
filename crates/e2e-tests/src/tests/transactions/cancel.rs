use crate::client::RelayerClient;
use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
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
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("test-original".to_string()),
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

        let cancel_result = self
            .relayer_client
            .sdk
            .transaction
            .cancel_transaction(transaction_id)
            .await
            .context("Failed to cancel transaction")?;

        if !cancel_result {
            return Err(anyhow::anyhow!("Cancel transaction failed"));
        }

        self.anvil_manager.mine_and_wait().await?;
        let mut attempts = 0;
        loop {
            if attempts > 10 {
                return Err(anyhow::anyhow!("Cancel transaction failed"));
            }
            let result = self.relayer_client.get_transaction_status(&send_result.id).await?;
            if result.status == TransactionStatus::Mined
                || result.status == TransactionStatus::Expired
            {
                break;
            } else {
                attempts += 1;
                self.anvil_manager.mine_and_wait().await?;
            }
        }

        let transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        if !transaction.is_noop {
            return Err(anyhow::anyhow!(
                "Expected the transaction to be a no-op {}",
                transaction_id
            ));
        }

        info!("[SUCCESS] Transaction {} cancel operation works correctly", transaction_id);

        Ok(())
    }
}
