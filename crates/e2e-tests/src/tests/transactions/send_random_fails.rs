use crate::tests::test_runner::TestRunner;
use anyhow::anyhow;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=send_random_fails
    /// RRELAYER_PROVIDERS="pkcs11" make run-test-debug TEST=send_random_fails
    pub async fn send_random_fails(&self) -> anyhow::Result<()> {
        info!("Testing simple eth transfer...");

        let relayer = self.create_and_fund_relayer("send-random-fails").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("send-random-fails".to_string()),
            blobs: None,
        };

        let relayer_client = self
            .relayer_client
            .client
            .transaction()
            .send_random(self.config.chain_id, &tx_request, None)
            .await;
        match relayer_client {
            Err(_) => Ok(()),
            Ok(_) => Err(anyhow!("Should not send random tx")),
        }
    }
}
