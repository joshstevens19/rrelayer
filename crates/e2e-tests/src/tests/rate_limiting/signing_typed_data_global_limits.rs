use crate::tests::test_runner::TestRunner;
use alloy::dyn_abi::TypedData;
use anyhow::{anyhow, Context};
use std::time::Duration;
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

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer1: {:?}", relayer);

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

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer2: {:?}", relayer);

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer3: {:?}", relayer);

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer4: {:?}", relayer);

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer5: {:?}", relayer);

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer6: {:?}", relayer);

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => successful_signing += 1,
            Err(_) => {}
        }

        if successful_signing != 3 {
            return Err(anyhow!("Signing typed data rate limiting not enforced should of got 3 but got {}", successful_signing));
        }

        info!("Successful signing operations before rate limit: {}", successful_signing);

        info!("Sleep for 60 seconds to allow the rate limit to expire");
        tokio::time::sleep(Duration::from_secs(60)).await;

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
            .await;

        match sign_result {
            Ok(_) => {}
            Err(_) => {
                return Err(anyhow!(
                    "Signing typed data should go through as rate limit expired"
                ));
            }
        }

        info!("Sleep for 60 seconds to allow the rate limit to expire so doesnt hurt next test");
        tokio::time::sleep(Duration::from_secs(60)).await;

        info!("Rate limiting mechanism verified");
        Ok(())
    }
}