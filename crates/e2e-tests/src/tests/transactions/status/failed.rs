use crate::tests::test_runner::TestRunner;
use alloy::primitives::U256;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// make run-test-debug TEST=transaction_status_failed
    pub async fn transaction_status_failed(&self) -> anyhow::Result<()> {
        info!("Testing transaction failed state...");

        let relayer = self.create_and_fund_relayer("failed-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let tx_request = RelayTransactionRequest {
            to: contract_address,
            value: TransactionValue::new(U256::ZERO),
            data: TransactionData::new(alloy::primitives::Bytes::from_static(&[
                0xde, 0xad, 0xbe, 0xef,
            ])), // Invalid function selector
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-failed".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await;

        match send_result {
            Ok(tx_response) => {
                return Err(anyhow::anyhow!(
                    "Transaction sent successfully, but should have failed: {:?}",
                    tx_response
                ));
            }
            Err(_) => {
                info!(
                    "[SUCCESS] Transaction was rejected at gas estimation (also valid failure scenario)"
                );
                Ok(())
            }
        }
    }
}
