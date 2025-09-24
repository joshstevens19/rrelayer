use crate::tests::test_runner::TestRunner;
use alloy::network::ReceiptResponse;
use anyhow::{anyhow, Context};
use rrelayer_core::common_types::EvmAddress;
use rrelayer_core::transaction::types::TransactionData;
use std::str::FromStr;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_send_eth_safe_proxy
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_send_eth_safe_proxy
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_send_eth_safe_proxy
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_send_eth_safe_proxy
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_send_eth_safe_proxy
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_send_eth_safe_proxy
    pub async fn transaction_send_eth_safe_proxy(&self) -> anyhow::Result<()> {
        info!("Testing simple eth transfer using Safe proxy...");

        let relayer = self.create_by_index_and_fund_relayer(80).await?;
        info!("Created relayer at index 80: {:?}", relayer);

        let safe_proxy_address =
            EvmAddress::new(self.contract_interactor.get_expected_safe_address_for_provider()?);
        info!("Funding Safe proxy contract at {} with 5 ETH", safe_proxy_address);
        self.fund_relayer(&safe_proxy_address, alloy::primitives::utils::parse_ether("5")?).await?;
        info!("Safe proxy contract funded successfully");

        let recipient = &self.config.anvil_accounts[1];
        let recipient_balance_before =
            self.contract_interactor.get_eth_balance(&recipient.into_address()).await?;
        info!("Sending ETH transfer to {} using Safe proxy", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                recipient,
                alloy::primitives::utils::parse_ether("4")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer via Safe proxy")?;

        info!("ETH transfer sent via Safe proxy: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        let expected_safe_address = safe_proxy_address;
        let expected_recipient = *recipient;

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

        // 4. Verify gas was consumed (indicating execution)
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

        let recipient_balance_after =
            self.contract_interactor.get_eth_balance(&expected_recipient.into_address()).await?;
        if recipient_balance_before > recipient_balance_after {
            return Err(anyhow!(
                "Recipient did not receive the expected ETH! Balance before: {}, Balance now: {}",
                alloy::primitives::utils::format_ether(recipient_balance_before),
                alloy::primitives::utils::format_ether(recipient_balance_after)
            ));
        }
        info!(
            "[SUCCESS] Recipient {} balance after Safe transfer: {} ETH",
            expected_recipient,
            alloy::primitives::utils::format_ether(recipient_balance_after)
        );

        let safe_balance_after =
            self.contract_interactor.get_eth_balance(&expected_safe_address.into_address()).await?;
        let eth_balance_after = alloy::primitives::utils::format_ether(safe_balance_after);
        if safe_balance_after != alloy::primitives::utils::parse_ether("1")? {
            return Err(anyhow!(
                "Safe proxy balance increased after Safe transfer! Expected: <= 0.5 ETH, Got: {}",
                eth_balance_after
            ));
        }
        info!(
            "[SUCCESS] Safe proxy {} balance after transfer: {} ETH",
            expected_safe_address,
            alloy::primitives::utils::format_ether(safe_balance_after)
        );

        info!("🎉 All Safe proxy validations passed!");

        Ok(())
    }
}
