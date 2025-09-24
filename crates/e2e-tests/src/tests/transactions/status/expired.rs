use crate::tests::test_runner::TestRunner;
use tracing::info;

impl TestRunner {
    //TODO! NEED TO THINK ABOUT HOW TO TEST EXPIRED
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_status_expired  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_status_expired
    pub async fn transaction_status_expired(&self) -> anyhow::Result<()> {
        info!("Testing transaction expired state...");

        Ok(())
    }
}
