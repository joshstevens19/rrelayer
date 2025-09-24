use crate::tests::test_runner::TestRunner;
use anyhow::{anyhow, Context};
use rrelayer_core::common_types::{EvmAddress, PagingContext};
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::TransactionData;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=allowlist_add
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=allowlist_add  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=allowlist_add
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=allowlist_add
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=allowlist_add
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=allowlist_add
    pub async fn allowlist_add(&self) -> anyhow::Result<()> {
        info!("Testing allowlist list operation...");

        let relayer = self.create_and_fund_relayer("allowlist-list-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        for i in 0..3 {
            let test_address = self.config.anvil_accounts[i];
            self.relayer_client
                .sdk
                .relayer
                .allowlist
                .add(&relayer.id, &test_address)
                .await
                .context("Failed to add address to allowlist")?;
        }

        let paging = PagingContext { limit: 10, offset: 0 };
        let allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &paging)
            .await
            .context("Failed to get allowlist")?;

        info!("[SUCCESS] Allowlist has {} addresses", allowlist.items.len());

        if allowlist.items.len() != 3 {
            return Err(anyhow::anyhow!(
                "Expected at 3 addresses in allowlist, but got {}",
                allowlist.items.len()
            ));
        }

        let items = allowlist
            .items
            .iter()
            .filter(|a| {
                *a == &self.config.anvil_accounts[0]
                    || *a == &self.config.anvil_accounts[1]
                    || *a == &self.config.anvil_accounts[2]
            })
            .collect::<Vec<&EvmAddress>>();
        if items.len() != allowlist.items.len() {
            return Err(anyhow::anyhow!(
                "Expected at {} addresses in allowlist, but got {}",
                allowlist.items.len(),
                items.len()
            ));
        }

        let tx_response = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(
                &relayer.id,
                &RelayTransactionRequest {
                    to: self.config.anvil_accounts[4],
                    value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                    data: TransactionData::empty(),
                    speed: Some(TransactionSpeed::Fast),
                    external_id: None,
                    blobs: None,
                },
                None,
            )
            .await;

        if tx_response.is_ok() {
            return Err(anyhow!("Should not be able to send transaction to none allowed address"));
        }

        for i in 0..3 {
            let test_address = self.config.anvil_accounts[i];
            let _ = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(
                    &relayer.id,
                    &RelayTransactionRequest {
                        to: test_address,
                        value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                        data: TransactionData::empty(),
                        speed: Some(TransactionSpeed::Fast),
                        external_id: None,
                        blobs: None,
                    },
                    None,
                )
                .await
                .context("Failed to send transaction to allowed address")?;
        }

        info!("[SUCCESS] Allowlist list operation works correctly");
        Ok(())
    }
}
