use anyhow::{anyhow, Context, Result};
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// make run-test-debug TEST=relayer_delete
    pub async fn relayer_delete(&self) -> Result<()> {
        info!("Testing relayer delete...");

        let relayer = self.create_and_fund_relayer("delete-test-relayer").await?;
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

        self.relayer_client.sdk.relayer.delete(&relayer.id).await?;

        let created_relayer = self.relayer_client.sdk.relayer.get(&relayer.id).await?;

        if created_relayer.is_some() {
            return Err(anyhow!("Relayer has not have been deleted"));
        }

        info!("[SUCCESS] Relayer delete functionality working correctly");
        Ok(())
    }
}
