use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::types::TransactionData;
use std::str::FromStr;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_data_validation
    pub async fn transaction_data_validation(&self) -> anyhow::Result<()> {
        info!("Testing transaction data validation...");

        let relayer = self.create_and_fund_relayer("data-validation-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let valid_data_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::from_str("0x1234abcd").unwrap(),
            )
            .await;

        if valid_data_result.is_err() {
            return Err(anyhow::anyhow!(
                "Valid hex data should be accepted, but got error: {:?}",
                valid_data_result.err()
            ));
        }

        let empty_data_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if empty_data_result.is_err() {
            return Err(anyhow::anyhow!(
                "Empty data should be accepted, but got error: {:?}",
                empty_data_result.err()
            ));
        }

        let result = TransactionData::from_str("0xGGGG");
        if result.is_ok() {
            return Err(anyhow::anyhow!(
                "Invalid hex data should return an error but got accepted"
            ));
        }

        info!("[SUCCESS] Transaction data validation working");
        Ok(())
    }
}
