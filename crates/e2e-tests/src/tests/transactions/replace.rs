use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
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
            gas_price_ceiling: None,
            expires_in_seconds: None,
        };

        let send_result = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        // Send returns once the transaction is queued. Broadcast happens on the
        // relayer's pending loop, so an immediate replace often still sees
        // PENDING. A pending replace rewrites that same row in place, and the
        // mined status is then MINED. This test asserts the in-mempool
        // competition (original REPLACED, new transaction MINED), so wait
        // until the original has been broadcast. Mining while waiting would
        // include the original before the replacement can compete.
        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let status = relayer
                .transaction()
                .get_status(transaction_id)
                .await?
                .context("Original transaction status not found")?;

            match status.status {
                TransactionStatus::INMEMPOOL => {
                    info!("Original transaction is in mempool; replacing it");
                    break;
                }
                TransactionStatus::PENDING => {
                    attempts += 1;
                    if attempts > 100 {
                        anyhow::bail!(
                            "Original transaction {} stayed PENDING and was never broadcast",
                            transaction_id
                        );
                    }
                }
                other => {
                    anyhow::bail!(
                        "Original transaction {} became {} before it could be replaced",
                        transaction_id,
                        other
                    );
                }
            }
        }

        let replacement_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.2")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-replacement".to_string()),
            blobs: None,
            gas_price_ceiling: None,
            expires_in_seconds: None,
        };

        let replace_result = relayer
            .transaction()
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

        let replace_transaction_id = replace_result
            .replace_transaction_id
            .context("Replace succeeded without a replacement transaction id")?;
        if replace_transaction_id == send_result.id {
            anyhow::bail!(
                "Replace updated transaction {} in place; expected a competing in-mempool transaction",
                send_result.id
            );
        }

        let first_transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        let replace_transaction =
            self.relayer_client.get_transaction(&replace_transaction_id).await?;

        if first_transaction.status != TransactionStatus::REPLACED {
            return Err(anyhow::anyhow!(format!(
                "First transaction {} is not replaced it is {}",
                first_transaction.id, first_transaction.status
            )));
        }

        if replace_transaction.status != TransactionStatus::MINED {
            return Err(anyhow::anyhow!(format!(
                "Replace transaction {} is not mined it is {}",
                replace_transaction.id, replace_transaction.status
            )));
        }

        self.relayer_client.sent_transaction_compare(replacement_request, replace_transaction)?;

        info!("[SUCCESS] Transaction replace operation works correctly");
        Ok(())
    }
}
