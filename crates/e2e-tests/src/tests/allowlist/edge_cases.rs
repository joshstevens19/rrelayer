use crate::tests::test_runner::TestRunner;
use rrelayer_core::common_types::PagingContext;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=allowlist_edge_cases
    pub async fn allowlist_edge_cases(&self) -> anyhow::Result<()> {
        info!("Testing allowlist edge cases...");

        let relayer = self.create_and_fund_relayer("allowlist-edge-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_address = self.config.anvil_accounts[1];

        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &test_address).await?;
        let duplicate_result =
            self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &test_address).await;

        match duplicate_result {
            Ok(_) => info!("Duplicate address add succeeded (graceful handling)"),
            Err(_) => {
                return Err(anyhow::anyhow!("Duplicate address add failed (graceful handling)"))
            }
        }

        let non_existent = self.config.anvil_accounts[4];
        let remove_result =
            self.relayer_client.sdk.relayer.allowlist.delete(&relayer.id, &non_existent).await;

        match remove_result {
            Ok(_) => info!("Remove non-existent succeeded (graceful handling)"),
            Err(_) => {
                return Err(anyhow::anyhow!("Remove non-existent failed (graceful handling)"))
            }
        }

        let allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &PagingContext::new(50, 0))
            .await?;

        if allowlist.items.len() != 1 {
            return Err(anyhow::anyhow!(
                "Allowlist should have 1 item, but got: {:?}",
                allowlist.items.len()
            ));
        }

        if allowlist.items[0] != test_address {
            return Err(anyhow::anyhow!(
                "Allowlist should have first item be test address, but got: {:?}",
                allowlist.items[0]
            ));
        }

        info!("[SUCCESS] Allowlist edge cases handled correctly");
        Ok(())
    }
}
