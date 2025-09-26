use crate::tests::test_runner::TestRunner;
use alloy::network::ReceiptResponse;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_gas_price_bumping
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_gas_price_bumping  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_gas_price_bumping
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_gas_price_bumping
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_gas_price_bumping
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_gas_price_bumping
    pub async fn transaction_gas_price_bumping(&self) -> anyhow::Result<()> {
        info!("Testing gas price bumping...");

        let relayer = self.create_and_fund_relayer("gas-bump-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("gas-bump-test".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send(&relayer.id, &tx_request, None).await?;

        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::INMEMPOOL {
                info!("Transaction reached InMempool with hash: {:?}", status.hash);
                break;
            }

            attempts += 1;
            if attempts > 20 {
                anyhow::bail!("Transaction did not reach InMempool");
            }
        }

        let transaction_before = self
            .relayer_client
            .sdk
            .transaction
            .get(&send_result.id)
            .await?
            .context("Transaction not found")?;
        let max_fee_per_gas_before = transaction_before
            .sent_with_max_fee_per_gas
            .context("transaction_before did not have sent_with_max_fee_per_gas")?;
        let sent_with_max_priority_before =
            transaction_before
                .sent_with_max_priority_fee_per_gas
                .context("transaction_before did not have sent_with_max_priority_fee_per_gas")?;

        // wait 10 seconds as gas bumping happens based on time
        tokio::time::sleep(Duration::from_secs(10)).await;

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        let transaction_after = self
            .relayer_client
            .sdk
            .transaction
            .get(&send_result.id)
            .await?
            .context("Transaction not found")?;
        let max_fee_per_gas_after = transaction_after
            .sent_with_max_fee_per_gas
            .context("transaction_after did not have sent_with_max_fee_per_gas")?;
        let sent_with_max_priority_after = transaction_after
            .sent_with_max_priority_fee_per_gas
            .context("transaction_after did not have sent_with_max_priority_fee_per_gas")?;

        if max_fee_per_gas_before == max_fee_per_gas_after {
            return Err(anyhow::anyhow!("Gas price did not bump max_fee"));
        }

        if sent_with_max_priority_before == sent_with_max_priority_after {
            return Err(anyhow::anyhow!("Gas price did not bump max_priority_fee"));
        }

        let transaction_status = self
            .relayer_client
            .sdk
            .transaction
            .get_status(&send_result.id)
            .await?
            .context("Transaction status not found")?
            .receipt
            .context("Transaction status did not have receipt")?;
        if !transaction_status.status() {
            return Err(anyhow::anyhow!("Transaction failed after gas bumping"));
        }

        info!("[SUCCESS] Gas price bumping mechanism verified");
        Ok(())
    }
}
