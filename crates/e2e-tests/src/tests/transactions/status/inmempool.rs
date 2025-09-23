use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_status_inmempool
    pub async fn transaction_status_inmempool(&self) -> anyhow::Result<()> {
        info!("Testing transaction inmempool state...");

        let relayer = self.create_and_fund_relayer("inmempool-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-inmempool".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Inmempool {
                if status.hash.is_none() {
                    return Err(anyhow::anyhow!("InMempool transaction should have hash"));
                }
                let hash = status.hash.unwrap();
                if hash != send_result.hash {
                    return Err(anyhow::anyhow!(
                        "InMempool transaction should match the sent transaction hash"
                    ));
                }

                if status.receipt.is_some() {
                    return Err(anyhow::anyhow!("InMempool transaction should not have receipt"));
                }
                info!("[SUCCESS] Transaction successfully reached InMempool state");
                return Ok(());
            }

            attempts += 1;
            if attempts > 10 {
                anyhow::bail!("Transaction did not reach InMempool state in time");
            }
        }
    }
}
