use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_status_mined
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_status_mined  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_status_mined
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_status_mined
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_status_mined
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_status_mined
    pub async fn transaction_status_mined(&self) -> anyhow::Result<()> {
        info!("Testing transaction mined state...");

        let relayer = self.create_and_fund_relayer("mined-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-mined".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send(&relayer.id, &tx_request, None).await?;

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
                break;
            }
        }

        self.mine_and_wait().await?;

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

            if status.status == TransactionStatus::MINED {
                if status.hash.is_none() {
                    return Err(anyhow::anyhow!("Mined transaction should have hash"));
                }

                let hash = status.hash.unwrap();
                if hash != send_result.hash {
                    return Err(anyhow::anyhow!(
                        "Mined transaction should match the sent transaction hash"
                    ));
                }

                if status.receipt.is_none() {
                    return Err(anyhow::anyhow!("Mined transaction should have receipt"));
                }
                let receipt = status.receipt.unwrap();
                info!("Transaction receipt: {:?}", receipt);
                if !receipt.inner.inner.status() {
                    return Err(anyhow::anyhow!("Mined transaction should have a success as true"));
                }

                info!("[SUCCESS] Transaction successfully reached Mined state");
                return Ok(());
            }

            attempts += 1;
            if attempts > 10 {
                anyhow::bail!("Transaction did not reach Mined state in time");
            }
        }
    }
}
