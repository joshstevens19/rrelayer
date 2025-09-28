use crate::tests::test_runner::TestRunner;
use anyhow::anyhow;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=rate_limiting_signing_message_global_limits
    pub async fn rate_limiting_signing_message_global_limits(&self) -> anyhow::Result<()> {
        info!("Testing rate limiting signing message global enforcement...");

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer1: {:?}", relayer);

        let relay_key = Some(self.config.anvil_accounts[0].to_string());

        let mut successful_signing = 0;

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer2: {:?}", relayer);

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer3: {:?}", relayer);

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer4: {:?}", relayer);

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer5: {:?}", relayer);

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer6: {:?}", relayer);

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        if successful_signing != 3 {
            return Err(anyhow!(
                "Signing message rate limiting not enforced should of got 3 but got {}",
                successful_signing
            ));
        }

        info!("Successful signing operations before rate limit: {}", successful_signing);

        info!("Sleep for 60 seconds to allow the rate limit to expire");
        tokio::time::sleep(Duration::from_secs(60)).await;

        let sign_result = relayer.sign().text("Hello, RRelayer!", relay_key.clone()).await;

        match sign_result {
            Ok(_) => {}
            Err(_) => {
                return Err(anyhow!("Signing message should go through as rate limit expired"));
            }
        }

        info!("Sleep for 60 seconds to allow the rate limit to expire so doesnt hurt next test");
        tokio::time::sleep(Duration::from_secs(60)).await;

        info!("Rate limiting mechanism verified");
        Ok(())
    }
}
