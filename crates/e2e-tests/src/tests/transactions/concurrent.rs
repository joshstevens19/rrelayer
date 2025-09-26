use crate::tests::test_runner::TestRunner;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::{Duration, Instant};
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_concurrent
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_concurrent  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_concurrent
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_concurrent
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_concurrent
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_concurrent
    pub async fn transaction_concurrent(&self) -> anyhow::Result<()> {
        info!("Testing concurrent transactions...");

        let relayer = self.create_and_fund_relayer("concurrent-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let mut tx_requests = Vec::new();
        for i in 0..50 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.000000005")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("concurrent-test-{}", i)),
                blobs: None,
            };
            tx_requests.push(tx_request);
        }

        info!("Sending {} transactions concurrently...", tx_requests.len());
        let mut handles = Vec::new();

        for (i, tx_request) in tx_requests.into_iter().enumerate() {
            let relayer_client = self.relayer_client.clone();
            let relayer_id = relayer.id;

            let handle = tokio::spawn(async move {
                let result =
                    relayer_client.sdk.transaction.send(&relayer_id, &tx_request, None).await;
                (i, result)
            });

            handles.push(handle);
        }

        let mut transaction_ids = Vec::new();
        let mut successful = 0;
        let mut failed = 0;

        for handle in handles {
            let (i, result) = handle.await?;
            match result {
                Ok(send_result) => {
                    transaction_ids.push(send_result.id);
                    successful += 1;
                }
                Err(e) => {
                    info!("Transaction {} failed: {}", i, e);
                    failed += 1;
                }
            }
        }

        info!("Concurrent transactions - Successful: {}, Failed: {}", successful, failed);

        if failed != 0 {
            return Err(anyhow::anyhow!("Concurrent transactions failed - {}", failed));
        }

        self.mine_and_wait().await?;
        info!("Waiting for all transactions to reach mined status...");

        let timeout = Duration::from_secs(180);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for transactions to be mined"));
            }

            let mut all_mined = true;
            for tx_id in &transaction_ids {
                if let Some(tx) = self.relayer_client.sdk.transaction.get(tx_id).await? {
                    if tx.status != TransactionStatus::MINED {
                        all_mined = false;
                        break;
                    }
                } else {
                    all_mined = false;
                    break;
                }
            }

            if all_mined {
                info!("All {} transactions are now mined", transaction_ids.len());
                break;
            }

            self.mine_and_wait().await?;
        }

        info!("[SUCCESS] Concurrent transaction handling verified");
        Ok(())
    }
}
