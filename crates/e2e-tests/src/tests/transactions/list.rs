use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::common_types::PagingContext;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_list
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_list  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_list
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_list
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_list
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_list
    pub async fn transaction_list(&self) -> anyhow::Result<()> {
        info!("Testing transaction list operation...");

        let relayer = self.create_and_fund_relayer("tx-list-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        for i in 1..=3 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.1")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::FAST),
                external_id: Some(format!("test-list-{}", i)),
                blobs: None,
            };

            let _ = relayer
                .transaction()
                .send(&tx_request, None)
                .await
                .context("Failed to send transaction")?;
        }

        let paging = PagingContext { limit: 10, offset: 0 };
        let relayer_transactions = relayer
            .transaction()
            .get_all(&paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!("[SUCCESS] Found {} transactions for relayer", relayer_transactions.items.len());

        if relayer_transactions.items.len() != 3 {
            return Err(anyhow::anyhow!(
                "Expected at 3 transactions, but got {}",
                relayer_transactions.items.len()
            ));
        }

        info!("[SUCCESS] Transaction list operation works correctly");
        Ok(())
    }
}
