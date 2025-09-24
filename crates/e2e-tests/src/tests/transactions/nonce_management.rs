use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::{Duration, Instant};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_nonce_management
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_nonce_management
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_nonce_management
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_nonce_management
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_nonce_management
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_nonce_management
    pub async fn transaction_nonce_management(&self) -> anyhow::Result<()> {
        info!("Testing transaction nonce management...");

        let relayer = self.create_and_fund_relayer("nonce-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let mut transaction_ids = Vec::new();

        for i in 0..50 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.000000005")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("nonce-test-{}", i)),
                blobs: None,
            };

            let send_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer.id, &tx_request, None)
                .await?;

            transaction_ids.push(send_result.id);
        }

        let mut nonces = Vec::new();
        for tx_id in &transaction_ids {
            if let Some(tx) = self.relayer_client.sdk.transaction.get_transaction(tx_id).await? {
                nonces.push(tx.nonce.into_inner());
            }
        }

        nonces.sort();

        for i in 1..nonces.len() {
            if nonces[i] != nonces[i - 1] + 1 {
                return Err(anyhow::anyhow!(
                    "Nonces should be sequential, but nonce {} ({}) != previous nonce {} ({}) + 1",
                    i,
                    nonces[i],
                    i - 1,
                    nonces[i - 1]
                ));
            }
        }

        self.mine_and_wait().await?;
        info!("Waiting for all transactions to reach mempool...");

        let timeout = Duration::from_secs(180);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for transactions to reach mempool"));
            }

            let mut all_in_mempool = true;
            for tx_id in &transaction_ids {
                if let Some(tx) = self.relayer_client.sdk.transaction.get_transaction(tx_id).await?
                {
                    if tx.status != TransactionStatus::Mined {
                        info!("Transaction {} not in mempool - status {}", tx_id, tx.status);
                        all_in_mempool = false;
                        break;
                    }
                } else {
                    info!("Transaction {} not in mempool - status", tx_id);
                    all_in_mempool = false;
                    break;
                }
            }

            if all_in_mempool {
                info!("All {} transactions are now in mempool", transaction_ids.len());
                break;
            }

            self.mine_and_wait().await?;
        }

        info!("[SUCCESS] Nonce management working correctly with sequential assignment");
        Ok(())
    }
}
