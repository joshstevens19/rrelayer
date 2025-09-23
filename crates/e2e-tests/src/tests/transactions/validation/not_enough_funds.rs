use crate::tests::test_runner::TestRunner;
use anyhow::anyhow;
use rrelayer_core::common_types::EvmAddress;
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_validation_not_enough_funds
    pub async fn transaction_validation_not_enough_funds(&self) -> anyhow::Result<()> {
        info!("Testing failed transaction handling not enough funds...");

        let relayer = self.create_and_fund_relayer("failure-test-relayer-funds").await?;
        info!("Created relayer: {:?}", relayer);

        let result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &EvmAddress::zero(),
                alloy::primitives::utils::parse_ether("1000")?.into(),
                TransactionData::empty(),
            )
            .await;

        match result {
            Ok(tx_response) => {
                info!("Potentially failing transaction sent: {:?}", tx_response);
                let final_status = self.wait_for_transaction_completion(&tx_response.0.id).await;
                if final_status.is_ok() {
                    return Err(anyhow!("Did not fail the transaction something went wrong..."));
                }
                info!("Failure test result: {:?}", final_status);
            }
            Err(e) => {
                info!("Transaction rejected as expected (insufficient funds): {}", e);
                // This is the expected outcome for insufficient funds
            }
        }

        Ok(())
    }
}
