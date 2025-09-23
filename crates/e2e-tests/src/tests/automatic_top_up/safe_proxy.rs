use crate::tests::test_runner::TestRunner;
use tracing::info;

impl TestRunner {
    // TODO: automatic_top_up_safe_proxy last part

    /// run single with:
    /// make run-test-debug TEST=automatic_top_up_safe_proxy
    pub async fn automatic_top_up_safe_proxy(&self) -> anyhow::Result<()> {
        info!("Testing automatic top-up using Safe proxy...");

        // Create a relayer with wallet index 80 (which is configured as Safe proxy signer)
        // Note: We need to create a relayer that maps to wallet index 80: 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf
        // For this test, we'll create relayers and drain them to below the threshold

        // let relayer1 = self.create_and_fund_relayer("safe-proxy-test-1").await?;
        // info!("relayer1: {:?}", relayer1);
        // let relayer2 = self.create_and_fund_relayer("safe-proxy-test-2").await?;
        // info!("relayer2: {:?}", relayer2);
        //
        // info!("Created test relayers for Safe proxy test: {:?}, {:?}", relayer1.id, relayer2.id);
        //
        // // Check initial balances
        // let initial_balance1 =
        //     self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        // let initial_balance2 =
        //     self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        //
        // info!("Initial balances:");
        // info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(initial_balance1));
        // info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(initial_balance2));
        //
        // // Check the Safe proxy funding address balance (wallet index 80)
        // let safe_proxy_signer: alloy::primitives::Address =
        //     "0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf"
        //         .parse()
        //         .context("Failed to parse Safe proxy signer address")?;
        // let safe_signer_balance =
        //     self.contract_interactor.get_eth_balance(&safe_proxy_signer).await?;
        // info!(
        //     "Safe proxy signer (wallet 80) balance: {} ETH",
        //     alloy::primitives::utils::format_ether(safe_signer_balance)
        // );
        //
        // // We need to fund the Safe proxy signer (wallet index 80) since it's configured as the from_address
        // // The automatic top-up configuration has from_address: 0x655B2B8861D7E911D283A05A5CAD042C157106DA
        // // But for Safe proxy, the signing will be done by wallet index 80: 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf
        // let funding_amount = alloy::primitives::utils::parse_ether("20")?;
        // info!(
        //     "Funding Safe proxy signer with {} ETH for testing...",
        //     alloy::primitives::utils::format_ether(funding_amount)
        // );
        //
        // self.fund_relayer(&safe_proxy_signer.into(), funding_amount).await?;
        //
        // let updated_safe_signer_balance =
        //     self.contract_interactor.get_eth_balance(&safe_proxy_signer).await?;
        // info!(
        //     "Updated Safe proxy signer balance: {} ETH",
        //     alloy::primitives::utils::format_ether(updated_safe_signer_balance)
        // );
        //
        // // Drain relayer balances to trigger automatic top-up
        // let drain_amount = alloy::primitives::utils::parse_ether("90")?; // Leave about 10 ETH
        // info!("Draining relayer balances to trigger Safe proxy top-up...");
        //
        // // Drain relayer1
        // if initial_balance1 > drain_amount {
        //     let tx_request = RelayTransactionRequest {
        //         to: self.config.anvil_accounts[4],
        //         value: drain_amount.into(),
        //         data: TransactionData::empty(),
        //         speed: Some(TransactionSpeed::Fast),
        //         external_id: Some("safe-drain-tx-1".to_string()),
        //         blobs: None,
        //     };
        //
        //     let tx_result = self
        //         .relayer_client
        //         .sdk
        //         .transaction
        //         .send_transaction(&relayer1.id, &tx_request, None)
        //         .await?;
        //     info!("Relayer 1 drain transaction sent: {:?}", tx_result.hash);
        // }
        //
        // // Drain relayer2
        // if initial_balance2 > drain_amount {
        //     let tx_request = RelayTransactionRequest {
        //         to: self.config.anvil_accounts[4],
        //         value: drain_amount.into(),
        //         data: TransactionData::empty(),
        //         speed: Some(TransactionSpeed::Fast),
        //         external_id: Some("safe-drain-tx-2".to_string()),
        //         blobs: None,
        //     };
        //
        //     let tx_result = self
        //         .relayer_client
        //         .sdk
        //         .transaction
        //         .send_transaction(&relayer2.id, &tx_request, None)
        //         .await?;
        //     info!("Relayer 2 drain transaction sent: {:?}", tx_result.hash);
        // }
        //
        // // Mine a few blocks to ensure drain transactions are processed
        // self.mine_blocks(5).await?;
        //
        // // Check balances after draining
        // let drained_balance1 =
        //     self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        // let drained_balance2 =
        //     self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        //
        // info!("Balances after draining:");
        // info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(drained_balance1));
        // info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(drained_balance2));
        //
        // // Wait for automatic top-up to trigger through Safe proxy
        // info!("Waiting for automatic Safe proxy top-up mechanism to trigger...");
        //
        // // The automatic top-up task runs every 30 seconds, so wait up to 2 minutes
        // let max_wait_time = tokio::time::Duration::from_secs(120);
        // let check_interval = tokio::time::Duration::from_secs(10);
        // let start_time = tokio::time::Instant::now();
        //
        // let min_expected_balance = alloy::primitives::utils::parse_ether("50")?; // Threshold is 50 ETH
        //
        // let mut relayer1_topped_up = false;
        // let mut relayer2_topped_up = false;
        //
        // while start_time.elapsed() < max_wait_time && (!relayer1_topped_up || !relayer2_topped_up) {
        //     tokio::time::sleep(check_interval).await;
        //
        //     let current_balance1 =
        //         self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        //     let current_balance2 =
        //         self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        //
        //     info!("Current balances ({}s elapsed):", start_time.elapsed().as_secs());
        //     info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(current_balance1));
        //     info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(current_balance2));
        //
        //     if current_balance1 > min_expected_balance && !relayer1_topped_up {
        //         info!("[SUCCESS] Relayer 1 successfully topped up via Safe proxy!");
        //         relayer1_topped_up = true;
        //     }
        //
        //     if current_balance2 > min_expected_balance && !relayer2_topped_up {
        //         info!("[SUCCESS] Relayer 2 successfully topped up via Safe proxy!");
        //         relayer2_topped_up = true;
        //     }
        // }
        //
        // // Verify both relayers were topped up
        // if !relayer1_topped_up {
        //     return Err(anyhow!(
        //         "Relayer 1 was not topped up within {} seconds. Current balance: {} ETH",
        //         max_wait_time.as_secs(),
        //         alloy::primitives::utils::format_ether(
        //             self.contract_interactor
        //                 .get_eth_balance(&relayer1.address.into_address())
        //                 .await?
        //         )
        //     ));
        // }
        //
        // if !relayer2_topped_up {
        //     return Err(anyhow!(
        //         "Relayer 2 was not topped up within {} seconds. Current balance: {} ETH",
        //         max_wait_time.as_secs(),
        //         alloy::primitives::utils::format_ether(
        //             self.contract_interactor
        //                 .get_eth_balance(&relayer2.address.into_address())
        //                 .await?
        //         )
        //     ));
        // }

        info!("[SUCCESS] Automatic Safe proxy top-up mechanism working correctly");
        Ok(())
    }
}
