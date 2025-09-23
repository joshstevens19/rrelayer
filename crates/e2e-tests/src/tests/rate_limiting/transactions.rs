use crate::tests::test_runner::TestRunner;
use anyhow::anyhow;
use rrelayer_core::transaction::types::TransactionData;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=rate_limiting_transaction
    pub async fn rate_limiting_transaction(&self) -> anyhow::Result<()> {
        info!("Testing rate limiting transaction enforcement...");

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer: {:?}", relayer);

        let relay_key = Some(self.config.anvil_accounts[0].to_string());

        let mut successful_transactions = 0;

        for i in 0..5 {
            let tx_result = self
                .relayer_client
                .send_transaction_with_rate_limit_key(
                    &relayer.id,
                    &self.config.anvil_accounts[1],
                    alloy::primitives::utils::parse_ether("0.5")?.into(),
                    TransactionData::empty(),
                    relay_key.clone(),
                )
                .await;

            match tx_result {
                Ok(_) => successful_transactions += 1,
                Err(_) => {}
            }
        }
        if successful_transactions != 1 {
            return Err(anyhow!("Sending transactions rate limiting not enforced"));
        }

        self.mine_blocks(1).await?;
        info!("Successful transactions before rate limit: {}", successful_transactions);

        info!("Sleep for 60 seconds to allow the rate limit to expire");
        tokio::time::sleep(Duration::from_secs(60)).await;

        let tx_result = self
            .relayer_client
            .send_transaction_with_rate_limit_key(
                &relayer.id,
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

        info!("Rate limiting mechanism verified");
        Ok(())
    }
}
