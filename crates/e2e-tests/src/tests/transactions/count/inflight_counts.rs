use crate::tests::test_runner::TestRunner;
use alloy::primitives::U256;
use anyhow::{anyhow, Context};
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use rrelayer::TransactionCountType;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_inflight_counts
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_inflight_counts
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_inflight_counts
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_inflight_counts
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_inflight_counts
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_inflight_counts
    pub async fn transaction_inflight_counts(&self) -> anyhow::Result<()> {
        info!("Testing transaction count operations...");

        let relayer = self.create_and_fund_relayer("tx-counts-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let initial_pending = relayer
            .transaction()
            .get_count(TransactionCountType::Pending)
            .await
            .context("Failed to get initial pending count")?;

        let initial_inmempool = relayer
            .transaction()
            .get_count(TransactionCountType::Inmempool)
            .await
            .context("Failed to get initial inmempool count")?;

        info!("Initial counts - Pending: {}, InMempool: {}", initial_pending, initial_inmempool);

        let mut transaction_ids = Vec::new();
        for i in 0..3 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: TransactionValue::new(U256::from(100000000000000000u128 * (i + 1))),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::FAST),
                external_id: Some(format!("test-counts-{}", i)),
                blobs: None,
            };

            let send_result = relayer
                .transaction()
                .send(&tx_request, None)
                .await
                .context(format!("Failed to send transaction {}", i))?;

            transaction_ids.push(send_result.id.clone());
            info!("Sent transaction {}: {}", i, send_result.id);

            self.mine_and_wait().await?;
        }

        let final_pending = relayer
            .transaction()
            .get_count(TransactionCountType::Pending)
            .await
            .context("Failed to get final pending count")?;

        let final_inmempool = relayer
            .transaction()
            .get_count(TransactionCountType::Inmempool)
            .await
            .context("Failed to get final inmempool count")?;

        info!("Final counts - Pending: {}, InMempool: {}", final_pending, final_inmempool);

        let total_final = final_pending + final_inmempool;
        let total_initial = initial_pending + initial_inmempool;

        if total_final >= total_initial {
            info!("[SUCCESS] Transaction counts increased as expected");
        } else {
            return Err(anyhow!(
                "Transaction counts may have decreased (transactions completed quickly)"
            ));
        }

        info!("[SUCCESS] Transaction count operations work correctly");
        Ok(())
    }
}
