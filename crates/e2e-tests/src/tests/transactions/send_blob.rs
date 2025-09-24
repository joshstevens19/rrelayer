use crate::tests::test_runner::TestRunner;
use alloy::primitives::U256;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_send_blob
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_send_blob
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_send_blob
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_send_blob
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_send_blob
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_send_blob
    pub async fn transaction_send_blob(&self) -> anyhow::Result<()> {
        info!("Testing blob transaction handling...");

        let relayer = self.create_and_fund_relayer("blob-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let blob_data = vec![1u8; 131072]; // 128KB blob
        let hex_blob = format!("0x{}", alloy::hex::encode(&blob_data));

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: TransactionValue::new(U256::ZERO),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("blob-test".to_string()),
            blobs: Some(vec![hex_blob]),
        };

        let blob_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

        let result = self.wait_for_transaction_completion(&blob_result.id).await?;

        self.relayer_client.sent_transaction_compare(tx_request, result.0)?;

        Ok(())
    }
}
