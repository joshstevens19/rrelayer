use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_status_pending
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_status_pending  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_status_pending
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_status_pending
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_status_pending
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_status_pending
    pub async fn transaction_status_pending(&self) -> anyhow::Result<()> {
        info!("Testing transaction pending state...");

        let relayer = self.create_and_fund_relayer("pending-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-pending".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send(&relayer.id, &tx_request, None).await?;

        let status = self
            .relayer_client
            .sdk
            .transaction
            .get_status(&send_result.id)
            .await?
            .context("Transaction status not found")?;

        if status.status != TransactionStatus::PENDING {
            return Err(anyhow::anyhow!(
                "Expected transaction to be in Pending state, but got: {:?}",
                status.status
            ));
        }

        if status.receipt.is_some() {
            return Err(anyhow::anyhow!(
                "Pending transaction should not have receipt, but got receipt"
            ));
        }

        info!("[SUCCESS] Transaction stays in Pending state without mining");
        Ok(())
    }
}
