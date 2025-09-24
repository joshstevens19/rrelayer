use crate::tests::test_runner::TestRunner;
use alloy::network::ReceiptResponse;
use alloy::primitives::Address;
use anyhow::{anyhow, Context};
use rrelayer_core::common_types::EvmAddress;
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    /// RELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    /// RELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    /// RELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    /// RELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_send_contract_interaction_safe_proxy
    pub async fn transaction_send_contract_interaction_safe_proxy(&self) -> anyhow::Result<()> {
        info!("Testing contract interaction via Safe proxy...");

        let relayer = self.create_by_index_and_fund_relayer(80).await?;
        info!("Created Safe proxy relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        info!(
            "Sending contract interaction to deployed contract at {} via Safe proxy",
            contract_address
        );

        let is_deployed = self.contract_interactor.verify_contract_deployed().await?;
        if !is_deployed {
            return Err(anyhow::anyhow!("Contract verification failed - no code at address"));
        }
        info!("[SUCCESS] Contract verified as deployed with code at {}", contract_address);

        let relayer_balance =
            self.contract_interactor.get_eth_balance(&relayer.address.into_address()).await?;
        info!(
            "Relayer balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(relayer_balance)
        );

        let safe_proxy_address = self.contract_interactor.get_expected_safe_address_for_provider()?;
        let safe_balance_before =
            self.contract_interactor.get_eth_balance(&safe_proxy_address).await?;
        info!(
            "Safe proxy balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(safe_balance_before)
        );

        let calldata: TransactionData =
            TransactionData::raw_hex(&self.contract_interactor.encode_simple_call(42)?).unwrap();

        let tx_response = self
            .relayer_client
            .send_transaction(&relayer.id, &contract_address, TransactionValue::zero(), calldata)
            .await
            .context("Failed to send contract interaction via Safe proxy")?;

        info!("Contract interaction sent via Safe proxy: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        let expected_safe_address = EvmAddress::new(safe_proxy_address);

        if EvmAddress::new(result.1.to.unwrap()) != expected_safe_address {
            return Err(anyhow!(
                "Transaction was not sent to Safe proxy! Expected: {}, Got: {}",
                expected_safe_address,
                EvmAddress::new(result.1.to.unwrap())
            ));
        }
        info!("[SUCCESS] Transaction correctly sent to Safe proxy: {}", expected_safe_address);

        if EvmAddress::new(result.1.from) != relayer.address {
            return Err(anyhow!(
                "Transaction was not sent from the expected relayer! Expected: {}, Got: {}",
                relayer.address,
                EvmAddress::new(result.1.from)
            ));
        }
        info!("[SUCCESS] Transaction correctly sent from relayer: {}", relayer.address);

        if !result.1.status() {
            return Err(anyhow!("Safe proxy transaction failed on-chain"));
        }
        info!("[SUCCESS] Safe proxy transaction succeeded on-chain");

        if result.1.inner.gas_used > 0 {
            info!(
                "[SUCCESS] Gas was consumed (gas used: {}), indicating transaction execution",
                result.1.inner.gas_used
            );
        }

        if !result.1.inner.inner.logs().is_empty() {
            info!(
                "[SUCCESS] Transaction emitted {} log(s), indicating Safe execution",
                result.1.inner.inner.logs().len()
            );
            for (i, log) in result.1.inner.inner.logs().iter().enumerate() {
                info!("   Log {}: address={}, topics={}", i, log.address(), log.topics().len());
            }
        } else {
            info!("[WARNING]  No logs emitted - this might indicate the Safe execution didn't emit expected events");
        }

        info!("🎉 All Safe proxy contract interaction validations passed!");
        Ok(())
    }
}
