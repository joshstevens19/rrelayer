use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_replace
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_replace
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_replace
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_replace
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_replace
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_replace
    pub async fn transaction_replace(&self) -> anyhow::Result<()> {
        info!("Testing transaction replace operation...");

        let relayer = self.create_and_fund_relayer("tx-replace-relayer").await?;
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

        let replacement_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.2")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-replacement".to_string()),
            blobs: None,
        };

        let replace_result = self
            .relayer_client
            .sdk
            .transaction
            .replace_transaction(transaction_id, &replacement_request)
            .await
            .context("Failed to replace transaction")?;
        info!("[SUCCESS] Transaction replacement result: {}", replace_result);

        if !replace_result {
            return Err(anyhow::anyhow!("Replace transaction failed"));
        }

        self.anvil_manager.mine_block().await?;

        let transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        self.relayer_client.sent_transaction_compare(replacement_request, transaction)?;

        info!("[SUCCESS] Transaction replace operation works correctly");
        Ok(())
    }
}
