use crate::tests::test_runner::TestRunner;
use anyhow::anyhow;
use rrelayer::ApiSdkError;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

fn is_rate_limit_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<ApiSdkError>()
            .is_some_and(|error| matches!(error, ApiSdkError::RateLimitError))
    })
}

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=rate_limiting_transaction_global_limits
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=rate_limiting_transaction_global_limits
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=rate_limiting_transaction_global_limits
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=rate_limiting_transaction_global_limits
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=rate_limiting_transaction_global_limits
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=rate_limiting_transaction_global_limits
    pub async fn rate_limiting_transaction_global_limits(&self) -> anyhow::Result<()> {
        info!("Testing rate limiting transaction enforcement...");

        super::wait_for_rate_limit_window_headroom().await;

        let relay_key = Some(self.config.anvil_accounts[0].to_string());

        let mut successful_transactions = 0;
        let mut attempts = 0;

        while successful_transactions < 3 {
            attempts += 1;
            if attempts > 12 {
                return Err(anyhow!(
                    "Could not send 3 successful transactions before testing the global limit"
                ));
            }

            let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
            info!("allowed relayer attempt {}: {:?}", attempts, relayer);

            let tx_result = self
                .relayer_client
                .send_transaction_with_rate_limit_key(
                    relayer.id(),
                    &self.config.anvil_accounts[1],
                    alloy::primitives::utils::parse_ether("0.5")?.into(),
                    TransactionData::empty(),
                    relay_key.clone(),
                )
                .await;

            match tx_result {
                Ok(_) => successful_transactions += 1,
                Err(error) if is_rate_limit_error(&error) => {
                    return Err(anyhow!(
                        "Global transaction rate limit triggered before 3 successful transactions"
                    ));
                }
                Err(error) => {
                    info!("Skipping relayer that cannot send transaction for this test: {}", error);
                }
            }
        }

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("over-limit relayer: {:?}", relayer);

        let tx_result = self
            .relayer_client
            .send_transaction_with_rate_limit_key(
                relayer.id(),
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
                relay_key.clone(),
            )
            .await;

        match tx_result {
            Err(error) if is_rate_limit_error(&error) => {}
            Ok(_) => {
                return Err(anyhow!("Global transaction rate limiting was not enforced"));
            }
            Err(error) => {
                return Err(anyhow!("Expected global transaction rate limit error, got {}", error));
            }
        }

        self.mine_blocks(1).await?;
        info!("Successful transactions before rate limit: {}", successful_transactions);

        info!("Wait for the rate limit to expire");
        super::wait_for_rate_limit_reset().await;

        let tx_result = self
            .relayer_client
            .send_transaction_with_rate_limit_key(
                relayer.id(),
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
                relay_key.clone(),
            )
            .await;

        match tx_result {
            Ok(_) => {}
            Err(_) => {
                return Err(anyhow!(
                    "Sending transactions should go through as rate limit expired"
                ));
            }
        }

        info!("Wait for the rate limit to expire so doesnt hurt next test");
        super::wait_for_rate_limit_reset().await;

        info!("Rate limiting mechanism verified");
        Ok(())
    }
}
