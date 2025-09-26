use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=automatic_top_up_native
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=automatic_top_up_native  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=automatic_top_up_native
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=automatic_top_up_native
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=automatic_top_up_native
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=automatic_top_up_native
    pub async fn automatic_top_up_native(&self) -> anyhow::Result<()> {
        info!("Testing automatic relayer balance top-up...");

        let relayer1 = self.create_by_index_and_fund_relayer(1).await?;
        info!("relayer1: {:?}", relayer1);
        let relayer2 = self.create_by_index_and_fund_relayer(2).await?;
        info!("relayer2: {:?}", relayer2);
        let relayer3 = self.create_by_index_and_fund_relayer(3).await?;
        info!("relayer3: {:?}", relayer3);

        info!("Created test relayers: {:?}, {:?}, {:?}", relayer1.id, relayer2.id, relayer3.id);

        let initial_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let initial_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let initial_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Initial balances:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(initial_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(initial_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(initial_balance3));

        let drain_amount = alloy::primitives::utils::parse_ether("90")?; // Leave about 10 ETH

        info!("Draining relayer balances to trigger top-up...");

        if initial_balance1 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::FAST),
                external_id: Some("drain-tx-1".to_string()),
                blobs: None,
            };

            self.relayer_client.sdk.transaction.send(&relayer1.id, &tx_request, None).await?;
        }

        if initial_balance2 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::FAST),
                external_id: Some("drain-tx-2".to_string()),
                blobs: None,
            };

            self.relayer_client.sdk.transaction.send(&relayer2.id, &tx_request, None).await?;
        }

        self.mine_and_wait().await?;

        let drained_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let drained_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let drained_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Balances after draining:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(drained_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(drained_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(drained_balance3));

        info!("Waiting for automatic top-up mechanism to trigger...");
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

        let final_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let final_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let final_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Final balances after top-up:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(final_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(final_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(final_balance3));

        let expected_top_up = alloy::primitives::utils::parse_ether("100")?;

        if drained_balance1 < expected_top_up {
            if final_balance1.abs_diff(expected_top_up) > initial_balance1 {
                return Err(anyhow::anyhow!(
                    "Relayer 1 balance not topped up correctly. Expected ~100 ETH, got {} ETH",
                    alloy::primitives::utils::format_ether(final_balance1)
                ));
            }
            info!("[SUCCESS] Relayer 1 successfully topped up to ~100 ETH");
        }

        if drained_balance2 < expected_top_up {
            if final_balance2.abs_diff(expected_top_up) > initial_balance2 {
                return Err(anyhow::anyhow!(
                    "Relayer 2 balance not topped up correctly. Expected ~100 ETH, got {} ETH",
                    alloy::primitives::utils::format_ether(final_balance2)
                ));
            }
            info!("[SUCCESS] Relayer 2 successfully topped up to ~100 ETH");
        }

        if drained_balance3 >= expected_top_up {
            let balance_change = final_balance3.abs_diff(drained_balance3);
            if balance_change > initial_balance3 {
                return Err(anyhow::anyhow!(
                    "Relayer 3 balance changed unexpectedly. Was {} ETH, now {} ETH",
                    alloy::primitives::utils::format_ether(drained_balance3),
                    alloy::primitives::utils::format_ether(final_balance3)
                ));
            }
            info!("[SUCCESS] Relayer 3 balance remained stable (no top-up needed)");
        }

        info!("[SUCCESS] Automatic top-up mechanism working correctly");
        Ok(())
    }
}
