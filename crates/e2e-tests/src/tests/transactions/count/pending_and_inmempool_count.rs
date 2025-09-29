use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use rrelayer::TransactionCountType;
use rrelayer_core::transaction::types::TransactionData;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_pending_and_inmempool_count
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_pending_and_inmempool_count
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_pending_and_inmempool_count
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_pending_and_inmempool_count
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_pending_and_inmempool_count
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_pending_and_inmempool_count
    pub async fn transaction_pending_and_inmempool_count(&self) -> anyhow::Result<()> {
        info!("Testing pending count...");

        let relayer = self.create_and_fund_relayer("limits-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let pending_count = relayer
            .transaction()
            .get_count(TransactionCountType::Pending)
            .await
            .context("Failed to get pending count")?;

        if pending_count > 0 {
            return Err(anyhow!("New relayer should not have transaction pending"));
        }

        let inmempool_count = relayer
            .transaction()
            .get_count(TransactionCountType::Inmempool)
            .await
            .context("Failed to get inmempool count")?;

        if inmempool_count > 0 {
            return Err(anyhow!("New relayer should not have transaction inmempool"));
        }

        let send_count = 3;

        for i in 0..send_count {
            let tx_response = self
                .relayer_client
                .send_transaction(
                    relayer.id(),
                    &self.config.anvil_accounts[4],
                    alloy::primitives::utils::parse_ether("0.01")?.into(),
                    TransactionData::empty(),
                )
                .await?;

            info!("[SUCCESS] Sent transaction {}: {:?}", i + 1, tx_response);
        }

        let pending_count = relayer
            .transaction()
            .get_count(TransactionCountType::Pending)
            .await
            .context("Failed to get pending count")?;

        if pending_count == 0 {
            return Err(anyhow!("Expected some pending transactions but got none"));
        }

        self.mine_and_wait().await?;

        let pending_count = relayer
            .transaction()
            .get_count(TransactionCountType::Pending)
            .await
            .context("Failed to get pending count")?;

        if pending_count != 0 {
            return Err(anyhow!("Expected 0 pending transactions, got {}", pending_count));
        }

        let inmempool_count = relayer
            .transaction()
            .get_count(TransactionCountType::Inmempool)
            .await
            .context("Failed to get inmempool count")?;

        if inmempool_count == 0 {
            return Err(anyhow!("Expected some inmempool transactions but got none"));
        }

        self.mine_blocks(2).await?;

        let mut attempts = 0;
        loop {
            let inmempool_count = relayer
                .transaction()
                .get_count(TransactionCountType::Inmempool)
                .await
                .context("Failed to get inmempool count")?;

            attempts += 1;

            if inmempool_count != 0 {
                if attempts > 10 {
                    return Err(anyhow!(
                        "Expected 0 inmempool transactions, got {}",
                        inmempool_count
                    ));
                }
            } else {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}
