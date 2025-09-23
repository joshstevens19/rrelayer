use crate::tests::test_runner::TestRunner;
use tracing::info;

impl TestRunner {
    //TODO! NEED TO THINK ABOUT HOW TO TEST EXPIRED
    /// run single with:
    /// make run-test-debug TEST=transaction_status_expired
    pub async fn transaction_status_expired(&self) -> anyhow::Result<()> {
        info!("Testing transaction expired state...");

        Ok(())
    }
}
