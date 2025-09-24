use anyhow::{anyhow, Result};
use rrelayer_core::common_types::EvmAddress;
use tracing::info;

use crate::tests::test_runner::TestRunner;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=relayer_concurrent_creation
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=relayer_concurrent_creation
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=relayer_concurrent_creation
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=relayer_concurrent_creation
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=relayer_concurrent_creation
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=relayer_concurrent_creation
    pub async fn relayer_concurrent_creation(&self) -> Result<()> {
        info!("Testing concurrent relayer creation to verify deadlock fix...");

        let target_relayers = 50;
        info!("Creating {} relayers concurrently to test deadlock prevention", target_relayers);

        let start_time = std::time::Instant::now();

        let batch_size = 5;
        let mut all_relayers = Vec::new();

        for batch_start in (0..target_relayers).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size - 1, target_relayers - 1);
            info!("Creating concurrent batch {}-{}", batch_start, batch_end);

            let relayer_names: Vec<String> = (batch_start..=batch_end)
                .map(|i| format!("concurrent_test_relayer_{}", i))
                .collect();

            let batch_futures: Vec<_> =
                relayer_names.iter().map(|name| self.create_relayer(name)).collect();

            let batch_results = futures::future::try_join_all(batch_futures).await?;

            for (i, relayer) in batch_results.into_iter().enumerate() {
                let index = batch_start + i;
                info!(
                    "Successfully created concurrent relayer {} at position {}",
                    relayer.id, index
                );
                all_relayers.push(relayer);
            }
        }

        let duration = start_time.elapsed();
        info!(
            "Successfully created {} relayers in {:?} without deadlocks!",
            all_relayers.len(),
            duration
        );

        let mut addresses: std::collections::HashSet<EvmAddress> = std::collections::HashSet::new();
        for relayer in &all_relayers {
            if !addresses.insert(relayer.address) {
                return Err(anyhow!(
                    "Duplicate address found: {}. This indicates a race condition!",
                    relayer.address
                ));
            }
        }

        info!("[SUCCESS] All {} relayers have unique addresses", all_relayers.len());
        info!("[SUCCESS] Concurrent relayer creation deadlock fix verified successfully!");

        Ok(())
    }
}
