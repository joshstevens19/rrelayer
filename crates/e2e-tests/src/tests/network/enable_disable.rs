use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use rrelayer_core::common_types::EvmAddress;
use rrelayer_core::network::ChainId;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=network_disable_enable
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=network_disable_enable  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=network_disable_enable
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=network_disable_enable
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=network_disable_enable
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=network_disable_enable
    pub async fn network_disable_enable(&self) -> anyhow::Result<()> {
        info!("Testing network management APIs...");

        let all_networks = self
            .relayer_client
            .sdk
            .network
            .get_all_networks()
            .await
            .context("Failed to get all networks")?;
        info!("All networks: {} found", all_networks.len());

        if all_networks.len() != 1 {
            return Err(anyhow!(
                "Should only bring back 1 network brought back {}",
                all_networks.len()
            ));
        }

        let network = all_networks.first().context("No networks found")?;
        if network.disabled {
            return Err(anyhow!("Network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!(
                "Network provider URL does not match got {}",
                network.provider_urls.first().unwrap()
            ));
        }

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;
        info!("Enabled networks: {} found", enabled_networks.len());

        if enabled_networks.len() != 1 {
            return Err(anyhow!(
                "Should only bring back 1 enabled network brought back {}",
                enabled_networks.len()
            ));
        }

        let network = enabled_networks.first().unwrap();
        if network.disabled {
            return Err(anyhow!("Enabled network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Enabled network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Enabled network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Enabled network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Enabled network provider URL does not match"));
        }

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;
        info!("Disabled networks: {} found", disabled_networks.len());

        if disabled_networks.len() != 0 {
            return Err(anyhow!(
                "Should only bring back 0 disabled network brought back {}",
                enabled_networks.len()
            ));
        }

        self.relayer_client.sdk.network.disable_network(31337).await?;

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;

        if disabled_networks.len() != 1 {
            return Err(anyhow!(
                "Should only bring back 1 disabled network brought back {}",
                disabled_networks.len()
            ));
        }

        let network = disabled_networks.first().unwrap();
        if !network.disabled {
            return Err(anyhow!("Network should be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Network provider URL does not match"));
        }

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;

        if enabled_networks.len() != 0 {
            return Err(anyhow!(
                "Should only bring back 0 enabled network brought back {}",
                enabled_networks.len()
            ));
        }

        let relayer = self.create_and_fund_relayer("network-management").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_response = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(
                &relayer.id,
                &RelayTransactionRequest {
                    to: EvmAddress::zero(),
                    value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                    data: TransactionData::empty(),
                    speed: Some(TransactionSpeed::Fast),
                    external_id: None,
                    blobs: None,
                },
                None,
            )
            .await;

        if tx_response.is_ok() {
            return Err(anyhow!("Should not be able to send transaction to disabled network"));
        }

        self.relayer_client.sdk.network.enable_network(31337).await?;

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;

        if enabled_networks.len() != 1 {
            return Err(anyhow!(
                "Should only bring back 1 enabled network brought back {}",
                enabled_networks.len()
            ));
        }

        let network = enabled_networks.first().unwrap();
        if network.disabled {
            return Err(anyhow!("Enabled network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Enabled network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Enabled network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Enabled network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Enabled network provider URL does not match"));
        }

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;

        if disabled_networks.len() != 0 {
            return Err(anyhow!(
                "Should only bring back 0 disabled network brought back {}",
                enabled_networks.len()
            ));
        }

        info!("[SUCCESS] Network management APIs work correctly");
        Ok(())
    }
}
