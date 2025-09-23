use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::common_types::PagingContext;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=allowlist_remove
    pub async fn allowlist_remove(&self) -> anyhow::Result<()> {
        info!("Testing allowlist remove operation...");

        let relayer = self.create_and_fund_relayer("allowlist-remove-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_address = self.config.anvil_accounts[2];
        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .add(&relayer.id, &test_address)
            .await
            .context("Failed to add address to allowlist")?;

        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .delete(&relayer.id, &test_address)
            .await
            .context("Failed to remove address from allowlist")?;

        info!("[SUCCESS] Removed {} from allowlist", test_address.hex());

        let paging = PagingContext { limit: 10, offset: 0 };
        let updated_allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &paging)
            .await
            .context("Failed to get updated allowlist")?;

        let address_still_exists =
            updated_allowlist.items.iter().any(|addr| addr.hex() == test_address.hex());

        if address_still_exists {
            return Err(anyhow::anyhow!("Address still found in allowlist after deletion"));
        }

        info!("[SUCCESS] Allowlist remove operation works correctly");
        Ok(())
    }
}
