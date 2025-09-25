use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=allowlist_restrictions  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=allowlist_restrictions
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=allowlist_restrictions
    pub async fn allowlist_restrictions(&self) -> anyhow::Result<()> {
        info!("Testing allowlist restrictions...");

        let relayer = self.create_and_fund_relayer("allowlist-restriction-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let allowed_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.1")?.into(),
                TransactionData::empty(),
            )
            .await;

        if allowed_tx_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction to allowlisted address should succeed, but got error: {:?}",
                allowed_tx_result.err()
            ));
        }

        let forbidden_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[2],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if forbidden_tx_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction to non-allowlisted address should fail, but succeeded"
            ));
        }

        let relayer = self.create_and_fund_relayer("allowlist-restriction-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let forbidden_native_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.1")?.into(),
                TransactionData::empty(),
            )
            .await;

        if forbidden_native_tx_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction to allowlisted address with native value to disable_native_transfer address should fail, but succeeded"
            ));
        }

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let calldata: TransactionData =
            TransactionData::raw_hex(&self.contract_interactor.encode_simple_call(42)?).unwrap();

        let allowed_contract_tx_result = self
            .relayer_client
            .send_transaction(&relayer.id, &contract_address, TransactionValue::zero(), calldata)
            .await;

        if allowed_contract_tx_result.is_err() {
            return Err(anyhow::anyhow!(
                "Contract transaction to allowlisted address should succeed, but got error: {:?}",
                allowed_tx_result.err()
            ));
        }

        info!("[SUCCESS] Allowlist restrictions working correctly");
        Ok(())
    }
}
