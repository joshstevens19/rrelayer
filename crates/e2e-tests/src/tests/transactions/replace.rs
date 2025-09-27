use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
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
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-original".to_string()),
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

        let replacement_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.2")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-replacement".to_string()),
            blobs: None,
        };

        let replace_result = self
            .relayer_client
            .sdk
            .transaction
            .replace(transaction_id, &replacement_request, None)
            .await
            .context("Failed to replace transaction")?;
        info!("[SUCCESS] Transaction replacement result: {:?}", replace_result);

        if !replace_result.success {
            return Err(anyhow::anyhow!("Replace transaction failed"));
        }

        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;
        self.anvil_manager.mine_and_wait().await?;

        let first_transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        let replace_transaction = self
            .relayer_client
            .get_transaction(&replace_result.replace_transaction_id.unwrap())
            .await?;

        if first_transaction.status != TransactionStatus::REPLACED {
            return Err(anyhow::anyhow!("First transaction is not cancelled"));
        }

        if replace_transaction.status != TransactionStatus::MINED {
            return Err(anyhow::anyhow!("Replace transaction is not mined"));
        }

        self.relayer_client.sent_transaction_compare(replacement_request, replace_transaction)?;

        info!("[SUCCESS] Transaction replace operation works correctly");
        Ok(())
    }
}
