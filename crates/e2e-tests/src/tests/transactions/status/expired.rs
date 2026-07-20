use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
use tracing::info;

/// Restores an environment variable to its previous value on drop, so the
/// override cannot leak into other tests if this one panics or is cancelled
/// by the harness timeout.
struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=transaction_status_expired
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=transaction_status_expired
    pub async fn transaction_status_expired(&self) -> anyhow::Result<()> {
        info!("Testing transaction expired state...");

        let _expiration_guard = EnvVarGuard::set("RRELAYER_TRANSACTION_EXPIRATION_SECONDS", "1");

        let relayer = self.create_and_fund_relayer("expired-status-relayer").await?;

        relayer.update_max_gas_price(1).await?;

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-expired".to_string()),
            blobs: None,
        };

        let send_result = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to queue transaction")?;

        tokio::time::sleep(Duration::from_millis(1_200)).await;
        relayer.remove_max_gas_price().await?;

        for _ in 0..20 {
            self.mine_and_wait().await?;

            let status = relayer
                .transaction()
                .get_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::EXPIRED {
                let transaction = relayer
                    .transaction()
                    .get(&send_result.id)
                    .await?
                    .context("Transaction not found")?;

                if !transaction.is_noop {
                    anyhow::bail!("Expired transaction should be persisted as a no-op");
                }
                if transaction.to != transaction.from {
                    anyhow::bail!("Expired no-op should be sent to the relayer address");
                }
                if !transaction.value.is_zero() {
                    anyhow::bail!("Expired no-op should have zero value");
                }
                if !transaction.data.into_inner().is_empty() {
                    anyhow::bail!("Expired no-op should have empty data");
                }

                info!("[SUCCESS] Transaction reached Expired state");
                return Ok(());
            }

            if matches!(
                status.status,
                TransactionStatus::MINED | TransactionStatus::CONFIRMED | TransactionStatus::FAILED
            ) {
                anyhow::bail!(
                    "Expected transaction to expire, but got terminal status: {:?}",
                    status.status
                );
            }
        }

        anyhow::bail!("Transaction did not reach Expired state in time");
    }
}
