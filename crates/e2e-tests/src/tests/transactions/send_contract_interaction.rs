use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context, Result};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_send_contract_interaction
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_send_contract_interaction
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_send_contract_interaction
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_send_contract_interaction
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_send_contract_interaction
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_send_contract_interaction
    pub async fn transaction_send_contract_interaction(&self) -> Result<()> {
        info!("Testing contract interaction...");

        let relayer = self.create_and_fund_relayer("contract-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        info!("Sending contract interaction to deployed contract at {}", contract_address);

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

        let calldata: TransactionData =
            TransactionData::raw_hex(&self.contract_interactor.encode_simple_call(42)?).unwrap();

        let tx_response = self
            .relayer_client
            .send_transaction(&relayer.id, &contract_address, TransactionValue::zero(), calldata)
            .await
            .context("Failed to send contract interaction")?;

        info!("Contract interaction sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        info!("[SUCCESS] Contract interaction completed successfully");
        Ok(())
    }
}
