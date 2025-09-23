use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=gas_price
    pub async fn gas_price(&self) -> anyhow::Result<()> {
        info!("Testing gas price API...");

        let gas_prices = self
            .relayer_client
            .sdk
            .gas
            .get_gas_prices(self.config.chain_id)
            .await
            .context("Failed to get gas prices")?;

        info!("Gas prices for chain {}: {:?}", self.config.chain_id, gas_prices);

        if gas_prices.is_none() {
            return Err(anyhow!("Gas prices not found for the chain"));
        }

        Ok(())
    }
}
