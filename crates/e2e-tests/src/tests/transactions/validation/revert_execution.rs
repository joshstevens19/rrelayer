use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use std::str::FromStr;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_validation_revert_execution
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_validation_revert_execution
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_validation_revert_execution
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_validation_revert_execution
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_validation_revert_execution
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_validation_revert_execution
    pub async fn transaction_validation_revert_execution(&self) -> anyhow::Result<()> {
        info!("Testing failed transaction handling revert execution...");

        let relayer = self.create_and_fund_relayer("failure-test-relayer-revert").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &contract_address,
                TransactionValue::zero(),
                TransactionData::from_str("0xdeadbeef").unwrap(), // Invalid function selector - will revert
            )
            .await;

        match result {
            Ok(tx_response) => {
                info!("Contract revert transaction sent: {:?}", tx_response);
                let final_status = self.wait_for_transaction_completion(&tx_response.0.id).await;
                if final_status.is_ok() {
                    return Err(anyhow!("Did not fail the transaction something went wrong..."));
                }

                info!("Contract revert test result: {:?}", final_status);
            }
            Err(e) => {
                info!("Transaction rejected as expected (contract revert): {}", e);
                // This is also a valid outcome if gas estimation catches the revert
            }
        }

        Ok(())
    }
}
