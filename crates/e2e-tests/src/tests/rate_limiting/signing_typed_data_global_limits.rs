use crate::tests::test_runner::TestRunner;
use alloy::dyn_abi::TypedData;
use anyhow::{anyhow, Context};
use rrelayer::ApiSdkError;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=rate_limiting_signing_typed_data_global_limits
    pub async fn rate_limiting_signing_typed_data_global_limits(&self) -> anyhow::Result<()> {
        info!("Testing rate limiting signing typed data global enforcement...");

        super::wait_for_rate_limit_window_headroom().await;

        let relay_key = Some(self.config.anvil_accounts[0].to_string());

        let typed_data_json = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "Mail": [
                    {"name": "from", "type": "Person"},
                    {"name": "to", "type": "Person"},
                    {"name": "contents", "type": "string"}
                ],
                "Person": [
                    {"name": "name", "type": "string"},
                    {"name": "wallet", "type": "address"}
                ]
            },
            "primaryType": "Mail",
            "domain": {
                "name": "RRelayer Test",
                "version": "1",
                "chainId": self.config.chain_id,
                "verifyingContract": "0x0000000000000000000000000000000000000000"
            },
            "message": {
                "from": {
                    "name": "Alice",
                    "wallet": "0x1234567890123456789012345678901234567890"
                },
                "to": {
                    "name": "Bob",
                    "wallet": "0x0987654321098765432109876543210987654321"
                },
                "contents": "Hello from E2E test!"
            }
        });

        let typed_data: TypedData =
            serde_json::from_value(typed_data_json).context("Failed to create typed data")?;

        let mut successful_signing = 0;
        let mut attempts = 0;

        while successful_signing < 3 {
            attempts += 1;
            if attempts > 12 {
                return Err(anyhow!(
                    "Could not complete 3 successful typed-data signing operations before testing the global limit"
                ));
            }

            let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
            info!("allowed relayer attempt {}: {:?}", attempts, relayer);

            let sign_result = relayer.sign().typed_data(&typed_data, relay_key.clone()).await;

            match sign_result {
                Ok(_) => successful_signing += 1,
                Err(ApiSdkError::RateLimitError) => {
                    return Err(anyhow!(
                        "Global typed-data signing rate limit triggered before 3 successful operations"
                    ));
                }
                Err(error) => {
                    info!("Skipping relayer that cannot sign typed data for this test: {}", error);
                }
            }
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("over-limit relayer: {:?}", relayer);

        let sign_result = relayer.sign().typed_data(&typed_data, relay_key.clone()).await;

        match sign_result {
            Err(ApiSdkError::RateLimitError) => {}
            Ok(_) => {
                return Err(anyhow!("Global typed-data signing rate limiting was not enforced"));
            }
            Err(error) => {
                return Err(anyhow!(
                    "Expected global typed-data signing rate limit error, got {}",
                    error
                ));
            }
        }

        info!("Successful signing operations before rate limit: {}", successful_signing);

        info!("Wait for the rate limit to expire");
        super::wait_for_rate_limit_reset().await;

        let sign_result = relayer.sign().typed_data(&typed_data, relay_key.clone()).await;

        match sign_result {
            Ok(_) => {}
            Err(_) => {
                return Err(anyhow!("Signing typed data should go through as rate limit expired"));
            }
        }

        info!("Wait for the rate limit to expire so doesnt hurt next test");
        super::wait_for_rate_limit_reset().await;

        info!("Rate limiting mechanism verified");
        Ok(())
    }
}
