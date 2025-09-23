use crate::client::E2ETestConfig;
use crate::tests::test_runner::TestRunner;
use rrelayer_core::common_types::PagingContext;
use rrelayer_sdk::SDK;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=unauthenticated
    pub async fn unauthenticated(&self) -> anyhow::Result<()> {
        info!("Testing unauthenticated requests...");

        let config = E2ETestConfig::default();
        let sdk =
            SDK::new(config.rrelayer_base_url.clone(), "wrong".to_string(), "way".to_string());
        info!("Created SDK with wrong credentials");

        let auth_status = sdk.auth.test_auth().await;
        if auth_status.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be authenticated"));
        }

        let relay = sdk.relayer.create(31337, "yes").await;
        if relay.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to create a relayer"));
        }

        let relayers = sdk.relayer.get_all(Some(31337), &PagingContext::new(50, 0)).await;
        if relayers.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to get relayers"));
        }

        info!("[SUCCESS] Unauthenticated checked");
        Ok(())
    }
}
