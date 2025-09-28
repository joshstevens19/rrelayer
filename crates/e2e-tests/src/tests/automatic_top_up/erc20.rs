use crate::tests::test_runner::TestRunner;
use alloy::primitives::U256;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=automatic_top_up_erc20
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=automatic_top_up_erc20  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=automatic_top_up_erc20
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=automatic_top_up_erc20
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=automatic_top_up_erc20
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=automatic_top_up_erc20
    pub async fn automatic_top_up_erc20(&self) -> anyhow::Result<()> {
        info!("Testing automatic ERC-20 token top-up...");

        let token_address = self
            .contract_interactor
            .token_address()
            .ok_or_else(|| anyhow::anyhow!("ERC-20 token not deployed"))?;

        info!("Using ERC-20 token at address: {:?}", token_address);

        let relayer1 = self.create_by_index_and_fund_relayer(1).await?;
        info!("relayer1: {:?}", relayer1);
        let relayer2 = self.create_by_index_and_fund_relayer(2).await?;
        info!("relayer2: {:?}", relayer2);
        let relayer3 = self.create_by_index_and_fund_relayer(3).await?;
        info!("relayer3: {:?}", relayer3);

        info!(
            "Created test relayers: {:?}, {:?}, {:?}",
            relayer1.id(),
            relayer2.id(),
            relayer3.id()
        );

        let initial_balance1 = self
            .contract_interactor
            .get_token_balance(&relayer1.address().await.unwrap().into_address())
            .await?;
        let initial_balance2 = self
            .contract_interactor
            .get_token_balance(&relayer2.address().await.unwrap().into_address())
            .await?;
        let initial_balance3 = self
            .contract_interactor
            .get_token_balance(&relayer3.address().await.unwrap().into_address())
            .await?;

        info!("Initial ERC-20 token balances:");
        info!(
            "  Relayer 1: {} tokens",
            alloy::primitives::utils::format_units(initial_balance1, 18)?
        );
        info!(
            "  Relayer 2: {} tokens",
            alloy::primitives::utils::format_units(initial_balance2, 18)?
        );
        info!(
            "  Relayer 3: {} tokens",
            alloy::primitives::utils::format_units(initial_balance3, 18)?
        );

        info!("Waiting for automatic ERC-20 top-up mechanism to trigger...");
        tokio::time::sleep(Duration::from_secs(30)).await;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        let final_balance1 = self
            .contract_interactor
            .get_token_balance(&relayer1.address().await.unwrap().into_address())
            .await?;
        let final_balance2 = self
            .contract_interactor
            .get_token_balance(&relayer2.address().await.unwrap().into_address())
            .await?;
        let final_balance3 = self
            .contract_interactor
            .get_token_balance(&relayer3.address().await.unwrap().into_address())
            .await?;

        info!("Final ERC-20 token balances after top-up:");
        info!(
            "  Relayer 1: {} tokens",
            alloy::primitives::utils::format_units(final_balance1, 18)?
        );
        info!(
            "  Relayer 2: {} tokens",
            alloy::primitives::utils::format_units(final_balance2, 18)?
        );
        info!(
            "  Relayer 3: {} tokens",
            alloy::primitives::utils::format_units(final_balance3, 18)?
        );

        let expected_top_up = U256::from(500u64) * U256::from(10u64).pow(U256::from(18u64));

        if final_balance1.abs_diff(expected_top_up) > initial_balance1 {
            return Err(anyhow::anyhow!(
                "Relayer 1 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance1, 18)?
            ));
        }
        info!("[SUCCESS] Relayer 1 successfully topped up to ~500 tokens");

        if final_balance2.abs_diff(expected_top_up) > initial_balance2 {
            return Err(anyhow::anyhow!(
                "Relayer 2 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance2, 18)?
            ));
        }
        info!("[SUCCESS] Relayer 2 successfully topped up to ~500 tokens");

        if final_balance3.abs_diff(expected_top_up) > initial_balance3 {
            return Err(anyhow::anyhow!(
                "Relayer 3 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance3, 18)?
            ));
        }
        info!("[SUCCESS] Relayer 3 successfully topped up to ~500 tokens");

        info!("[SUCCESS] Automatic ERC-20 token top-up mechanism working correctly");
        Ok(())
    }
}
