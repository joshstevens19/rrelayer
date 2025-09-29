use anyhow::{anyhow, Result};
use rrelayer_core::{gas::GasPrice, transaction::types::TransactionData};
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_gas_configuration
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_gas_configuration
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_gas_configuration
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_gas_configuration
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_gas_configuration
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_gas_configuration
    pub async fn relayer_gas_configuration(&self) -> Result<()> {
        info!("Testing relayer gas configuration...");

        let relayer = self.create_and_fund_relayer("gas-config-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        relayer.update_eip1559_status(false).await?;

        let config_after_legacy = self.relayer_client.client.relayer().get(relayer.id()).await?;
        if let Some(config) = config_after_legacy {
            if config.relayer.eip_1559_enabled {
                return Err(anyhow!("Relayer should not be using EIP1559 but it is enabled"));
            }
        }

        relayer.update_eip1559_status(true).await?;

        let config_after_latest = self.relayer_client.client.relayer().get(relayer.id()).await?;
        if let Some(config) = config_after_latest {
            if !config.relayer.eip_1559_enabled {
                return Err(anyhow!("Relayer should be using EIP1559 but it is not enabled"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        relayer.update_max_gas_price(1000000).await?;

        let config_after_max = self.relayer_client.client.relayer().get(relayer.id()).await?;
        if let Some(config) = config_after_max {
            if let Some(max) = config.relayer.max_gas_price {
                if max != GasPrice::new(1000000) {
                    return Err(anyhow!(
                        "Relayer should have max gas price of 1000000, but got: {:?}",
                        max
                    ));
                }
            } else {
                return Err(anyhow!("Relayer should have a max gas price"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        let tx_result = self
            .relayer_client
            .send_transaction(
                relayer.id(),
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if tx_result.is_err() {
            return Err(anyhow!(
                "Transaction should succeed with gas configuration, but got error: {:?}",
                tx_result.err()
            ));
        }

        relayer.remove_max_gas_price().await?;

        let config_after_none = self.relayer_client.client.relayer().get(relayer.id()).await?;
        if let Some(config) = config_after_none {
            if config.relayer.max_gas_price.is_some() {
                return Err(anyhow!("Relayer should not have a max gas price"));
            }
        } else {
            return Err(anyhow!("Relayer should have a config"));
        }

        info!("[SUCCESS] Gas configuration changes working correctly");
        Ok(())
    }
}
