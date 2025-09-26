use anyhow::{anyhow, Context, Result};
use rrelayer_core::{common_types::PagingContext, transaction::types::TransactionData};
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_clone
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_clone
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_clone
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_clone
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_clone
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_clone
    pub async fn relayer_clone(&self) -> Result<()> {
        info!("Testing relayer clone...");

        let relayer = self.create_and_fund_relayer("clone-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let created_relayer = self
            .relayer_client
            .sdk
            .relayer
            .get(&relayer.id)
            .await?
            .context("Relayer should exist")?;

        if created_relayer.relayer.id != relayer.id {
            return Err(anyhow!("Relayer should exist"));
        }

        let cloned_relayer = self
            .relayer_client
            .sdk
            .relayer
            .clone(&relayer.id, 31337, "cloned-test-relayer")
            .await?;
        if cloned_relayer.id == relayer.id {
            return Err(anyhow!("Relayer should have been cloned and have its own ID"));
        }

        if cloned_relayer.address != relayer.address {
            return Err(anyhow!("Relayer should have been cloned and have the shared address"));
        }

        let recipient = &self.config.anvil_accounts[1];
        info!("Sending ETH transfer to {} from the new cloned one", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &cloned_relayer.id,
                recipient,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer")?;

        info!("ETH transfer sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        let paging = PagingContext { limit: 10, offset: 0 };
        let first_relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_all(&relayer.id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!(
            "[SUCCESS] Found {} transactions for first relayer",
            first_relayer_transactions.items.len()
        );

        let cloned_relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_all(&cloned_relayer.id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!(
            "[SUCCESS] Found {} transactions for cloned relayer",
            cloned_relayer_transactions.items.len()
        );

        if first_relayer_transactions.items.len() != 0 {
            return Err(anyhow!(
                "First relayer expected at 0 transactions, but got {}",
                first_relayer_transactions.items.len()
            ));
        }

        if cloned_relayer_transactions.items.len() != 1 {
            return Err(anyhow!(
                "Cloned relayer expected at 1 transactions, but got {}",
                cloned_relayer_transactions.items.len()
            ));
        }

        info!("[SUCCESS] Relayer clone functionality working correctly");

        Ok(())
    }
}
