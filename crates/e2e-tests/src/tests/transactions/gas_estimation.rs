use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_gas_estimation
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_gas_estimation  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_gas_estimation
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_gas_estimation
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_gas_estimation
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_gas_estimation
    pub async fn transaction_gas_estimation(&self) -> anyhow::Result<()> {
        info!("Testing gas estimation and cost validation...");
        let relayer = self.create_and_fund_relayer("gas-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let balance_before = self
            .contract_interactor
            .get_eth_balance(&relayer.address().await?.into_address())
            .await?;
        info!(
            "Relayer balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(balance_before)
        );

        let transfer_amount = alloy::primitives::utils::parse_ether("0.1")?;
        let tx_response = self
            .relayer_client
            .send_transaction(
                relayer.id(),
                &self.config.anvil_accounts[3],
                transfer_amount.into(),
                TransactionData::empty(),
            )
            .await?;

        info!("Gas estimation test transaction sent: {:?}", tx_response.0.id);

        let completed_tx = self.wait_for_transaction_completion(&tx_response.0.id).await?;
        info!("Transaction completed");

        let balance_after = self
            .contract_interactor
            .get_eth_balance(&relayer.address().await?.into_address())
            .await?;
        info!(
            "Relayer balance after transaction: {} ETH",
            alloy::primitives::utils::format_ether(balance_after)
        );

        let balance_diff = balance_before - balance_after;
        let expected_transfer = transfer_amount;
        let gas_cost = balance_diff - expected_transfer;

        info!("Transaction cost breakdown:");
        info!(
            "  Transfer amount: {} ETH",
            alloy::primitives::utils::format_ether(expected_transfer)
        );
        info!("  Gas cost: {} ETH", alloy::primitives::utils::format_ether(gas_cost));
        info!("  Total cost: {} ETH", alloy::primitives::utils::format_ether(balance_diff));

        let min_gas_cost = alloy::primitives::utils::parse_ether("0.00001")?;
        let max_gas_cost = alloy::primitives::utils::parse_ether("0.001")?;

        anyhow::ensure!(
            gas_cost >= min_gas_cost,
            "Gas cost seems unreasonably low: {} ETH (expected at least {} ETH)",
            alloy::primitives::utils::format_ether(gas_cost),
            alloy::primitives::utils::format_ether(min_gas_cost)
        );

        anyhow::ensure!(
            gas_cost <= max_gas_cost,
            "Gas cost seems unreasonably high: {} ETH (expected at most {} ETH)",
            alloy::primitives::utils::format_ether(gas_cost),
            alloy::primitives::utils::format_ether(max_gas_cost)
        );

        if let Some(gas_limit) = completed_tx.0.gas_limit {
            let gas_limit_value = gas_limit.into_inner();
            info!("Gas limit used: {}", gas_limit_value);

            anyhow::ensure!(
                gas_limit_value >= 21000u128,
                "Gas limit too low for ETH transfer: {} (expected at least 21,000)",
                gas_limit_value
            );

            anyhow::ensure!(
                gas_limit_value <= 100000u128,
                "Gas limit too high for simple ETH transfer: {} (expected at most 100,000)",
                gas_limit_value
            );
        }

        info!("[SUCCESS] Gas estimation validation passed:");
        info!("  - Gas cost is within reasonable bounds");
        info!("  - Transaction completed successfully");
        info!("  - Cost efficiency validated for simple transfers");

        Ok(())
    }
}
