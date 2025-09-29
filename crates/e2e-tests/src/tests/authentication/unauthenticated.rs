use crate::client::E2ETestConfig;
use crate::tests::test_runner::TestRunner;
use rrelayer::{Client, CreateClientAuth, CreateClientConfig};
use rrelayer_core::common_types::PagingContext;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=unauthenticated
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=unauthenticated  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=unauthenticated
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=unauthenticated
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=unauthenticated
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=unauthenticated
    pub async fn unauthenticated(&self) -> anyhow::Result<()> {
        info!("Testing unauthenticated requests...");

        let config = E2ETestConfig::default();
        let client = Client::new(CreateClientConfig {
            server_url: config.rrelayer_base_url.clone(),
            auth: CreateClientAuth { username: "wrong".to_string(), password: "way".to_string() },
        });
        info!("Created SDK with wrong credentials");

        let auth_status = client.authenticated().await;
        if auth_status.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be authenticated"));
        }

        let relay = client.relayer().create(&31337, "yes").await;
        if relay.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to create a relayer"));
        }

        let relayers = client.relayer().get_all(&PagingContext::new(50, 0), Some(31337)).await;
        if relayers.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to get relayers"));
        }

        info!("[SUCCESS] Unauthenticated checked");
        Ok(())
    }
}
