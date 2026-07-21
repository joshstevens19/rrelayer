use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionStatus};
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// A transient insufficient-funds failure at send time must never terminally fail a
    /// transaction: the nonce was reserved at admission, so dropping the transaction
    /// permanently strands that nonce and wedges every transaction queued behind it.
    /// Low balance is operator-fixable — once topped up, the queue must drain on its own.
    ///
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_insufficient_funds_at_send_retries
    pub async fn transaction_insufficient_funds_at_send_retries(&self) -> anyhow::Result<()> {
        info!("Testing send-time insufficient funds retries until topped up...");

        let relayer = self.create_and_fund_relayer("insufficient-funds-retry-relayer").await?;
        let relayer_address = relayer.address().await?;

        // Hold sends so both transactions are admitted (nonces reserved) while balance is healthy
        relayer.update_max_gas_price(1).await?;

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-insufficient-funds-head".to_string()),
            blobs: None,
        };
        let head_tx = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to queue head transaction")?;

        let follow_up_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.3")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-insufficient-funds-followup".to_string()),
            blobs: None,
        };
        let follow_up_tx = relayer
            .transaction()
            .send(&follow_up_request, None)
            .await
            .context("Failed to queue follow-up transaction")?;

        // Drain the relayer below the head transaction's cost, then release the sends
        self.set_relayer_balance(
            &relayer_address,
            alloy::primitives::utils::parse_ether("0.0001")?,
        )
        .await?;
        relayer.remove_max_gas_price().await?;

        // The send attempt now fails with insufficient funds. The transaction must stay
        // PENDING (retrying) — terminally failing it strands the reserved nonce forever.
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            let status = relayer
                .transaction()
                .get_status(&head_tx.id)
                .await?
                .context("Head transaction status not found")?;

            anyhow::ensure!(
                status.status != TransactionStatus::FAILED,
                "BUG: transaction was terminally failed on a transient insufficient-funds send \
                 error — its reserved nonce is stranded and the relayer queue is wedged"
            );
            anyhow::ensure!(
                status.status == TransactionStatus::PENDING,
                "Test setup broken: expected head transaction to sit PENDING on a drained \
                 balance, got {:?}",
                status.status
            );
        }

        // Operator fixes the balance — everything must drain with no manual intervention
        self.fund_relayer(&relayer_address, alloy::primitives::utils::parse_ether("10")?).await?;

        self.wait_for_transaction_completion(&head_tx.id)
            .await
            .context("Head transaction did not recover after balance top-up")?;

        self.wait_for_transaction_completion(&follow_up_tx.id)
            .await
            .context("Follow-up transaction stuck behind head nonce after top-up")?;

        // Nonce continuity: a fresh transaction must also mine (no gap was created)
        let continuity_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-insufficient-funds-continuity".to_string()),
            blobs: None,
        };
        let continuity_tx = relayer
            .transaction()
            .send(&continuity_request, None)
            .await
            .context("Failed to queue continuity transaction")?;

        self.wait_for_transaction_completion(&continuity_tx.id)
            .await
            .context("Continuity transaction did not mine — nonce sequence has a gap")?;

        info!("[SUCCESS] Insufficient funds at send retried until topped up, no nonce stranded");
        Ok(())
    }

    /// The cancel-of-pending no-op (which exists to consume the cancelled transaction's
    /// reserved nonce) must itself survive a drained balance: if the no-op send fails with
    /// insufficient funds it must retry, not be terminally failed — otherwise the safety
    /// net defeats itself and the nonce is stranded anyway.
    ///
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_cancel_noop_low_balance_retries
    pub async fn transaction_cancel_noop_low_balance_retries(&self) -> anyhow::Result<()> {
        info!("Testing cancel no-op retries on drained balance...");

        let relayer = self.create_and_fund_relayer("cancel-noop-lowbalance-relayer").await?;
        let relayer_address = relayer.address().await?;

        relayer.update_max_gas_price(1).await?;

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-cancel-noop-head".to_string()),
            blobs: None,
        };
        let head_tx = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to queue head transaction")?;

        let follow_up_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.3")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-cancel-noop-followup".to_string()),
            blobs: None,
        };
        let follow_up_tx = relayer
            .transaction()
            .send(&follow_up_request, None)
            .await
            .context("Failed to queue follow-up transaction")?;

        // Cancel the still-pending head: it converts in place to a nonce-preserving no-op
        let cancel_result = relayer
            .transaction()
            .cancel(&head_tx.id, None)
            .await
            .context("Failed to cancel head transaction")?;
        anyhow::ensure!(cancel_result.success, "Cancel of pending transaction failed");

        // Drain the balance below even the no-op's gas cost, then release the sends
        self.set_relayer_balance(&relayer_address, alloy::primitives::U256::from(1u64)).await?;
        relayer.remove_max_gas_price().await?;

        // The no-op send fails with insufficient funds — it must keep retrying
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            let status = relayer
                .transaction()
                .get_status(&head_tx.id)
                .await?
                .context("Cancelled transaction status not found")?;

            anyhow::ensure!(
                status.status != TransactionStatus::FAILED,
                "BUG: the cancel no-op was terminally failed on a transient insufficient-funds \
                 send error — the nonce it exists to consume is stranded"
            );
            anyhow::ensure!(
                status.status == TransactionStatus::PENDING,
                "Test setup broken: expected cancel no-op to sit PENDING on a drained balance, \
                 got {:?}",
                status.status
            );
        }

        self.fund_relayer(&relayer_address, alloy::primitives::utils::parse_ether("10")?).await?;

        let cancelled = self
            .wait_for_transaction_terminal(&head_tx.id)
            .await
            .context("Cancel no-op did not reach a terminal state after top-up")?;
        anyhow::ensure!(
            cancelled.is_noop,
            "Cancelled transaction should have completed as a no-op"
        );

        self.wait_for_transaction_completion(&follow_up_tx.id)
            .await
            .context("Follow-up transaction stuck behind cancelled nonce after top-up")?;

        info!("[SUCCESS] Cancel no-op retried on drained balance and recovered after top-up");
        Ok(())
    }

    /// If the relayer's key is used OUTSIDE the relayer (consuming its next nonce),
    /// the queued transaction's send fails with 'nonce too low'. Recovery must
    /// reassign it a fresh nonce and drain the whole queue - never wedge it.
    ///
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_external_nonce_displacement_recovers
    pub async fn transaction_external_nonce_displacement_recovers(&self) -> anyhow::Result<()> {
        info!("Testing external nonce displacement recovery...");

        let relayer = self.create_and_fund_relayer("nonce-displacement-relayer").await?;
        let relayer_address = relayer.address().await?;

        // Hold sends so both transactions are admitted with reserved nonces
        relayer.update_max_gas_price(1).await?;

        let head_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.2")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-displacement-head".to_string()),
            blobs: None,
        };
        let head_tx = relayer
            .transaction()
            .send(&head_request, None)
            .await
            .context("Failed to queue head transaction")?;

        let follow_up_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-displacement-followup".to_string()),
            blobs: None,
        };
        let follow_up_tx = relayer
            .transaction()
            .send(&follow_up_request, None)
            .await
            .context("Failed to queue follow-up transaction")?;

        // Consume the head transaction's reserved nonce from OUTSIDE the relayer
        self.send_impersonated_transaction(
            &relayer_address,
            &self.config.anvil_accounts[1],
            alloy::primitives::utils::parse_ether("0.01")?,
        )
        .await?;

        relayer.remove_max_gas_price().await?;

        // The head send hits 'nonce too low'; recovery must reassign a fresh nonce
        // (after confirming via receipt that OUR broadcast did not consume it) and
        // every queued transaction must still mine
        self.wait_for_transaction_completion(&head_tx.id)
            .await
            .context("Displaced head transaction did not recover with a fresh nonce")?;

        self.wait_for_transaction_completion(&follow_up_tx.id)
            .await
            .context("Follow-up transaction stuck after external nonce displacement")?;

        let continuity_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.05")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-displacement-continuity".to_string()),
            blobs: None,
        };
        let continuity_tx = relayer
            .transaction()
            .send(&continuity_request, None)
            .await
            .context("Failed to queue continuity transaction")?;
        self.wait_for_transaction_completion(&continuity_tx.id)
            .await
            .context("Continuity transaction did not mine after displacement recovery")?;

        info!("[SUCCESS] External nonce displacement recovered, no nonce stranded");
        Ok(())
    }

    /// A transaction that reverts ON-CHAIN (state changed between admission and
    /// execution) consumes its nonce via the mined revert and resolves FAILED. The
    /// queue must move straight on to the next transaction.
    ///
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=transaction_onchain_revert_does_not_strand_nonce
    pub async fn transaction_onchain_revert_does_not_strand_nonce(&self) -> anyhow::Result<()> {
        use alloy::signers::local::PrivateKeySigner;

        info!("Testing on-chain revert nonce continuity...");

        let relayer = self.create_and_fund_relayer("mined-revert-relayer").await?;
        let relayer_address = relayer.address().await?;

        let token = self.contract_interactor.token_address().context("Test token not deployed")?;
        let deployer_key = self.config.anvil_private_keys[0].clone();
        let deployer_signer: PrivateKeySigner = deployer_key.parse()?;
        let deployer_address = deployer_signer.address();

        // Deployer approves the relayer; the approval survives the balance drain below
        let amount = alloy::primitives::U256::from(100u64);
        self.contract_interactor
            .approve_tokens(&relayer_address.into_address(), amount, &deployer_key)
            .await?;
        self.mine_and_wait().await?;

        // Hold sends, then admit the transferFrom while the deployer still has tokens
        // (gas estimation passes)
        relayer.update_max_gas_price(1).await?;

        let calldata = self.contract_interactor.encode_token_transfer_from(
            &deployer_address,
            &self.config.anvil_accounts[1].into_address(),
            amount,
        );
        let revert_request = RelayTransactionRequest {
            to: token,
            value: alloy::primitives::U256::ZERO.into(),
            data: TransactionData::new(calldata.into()),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-mined-revert".to_string()),
            blobs: None,
        };
        let revert_tx = relayer
            .transaction()
            .send(&revert_request, None)
            .await
            .context("Failed to queue transferFrom transaction")?;

        let follow_up_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("test-mined-revert-followup".to_string()),
            blobs: None,
        };
        let follow_up_tx = relayer
            .transaction()
            .send(&follow_up_request, None)
            .await
            .context("Failed to queue follow-up transaction")?;

        // Drain the deployer's token balance so the queued transferFrom reverts at
        // execution time (nodes do not simulate on submission)
        let deployer_balance =
            self.contract_interactor.get_token_balance(&deployer_address).await?;
        self.contract_interactor
            .transfer_tokens(
                &self.config.anvil_accounts[2].into_address(),
                deployer_balance,
                &deployer_key,
            )
            .await?;
        self.mine_and_wait().await?;

        relayer.remove_max_gas_price().await?;

        // The transferFrom broadcasts, mines as reverted, and must resolve FAILED
        let mut saw_failed = false;
        for _ in 0..30 {
            self.mine_and_wait().await?;
            let status = relayer
                .transaction()
                .get_status(&revert_tx.id)
                .await?
                .context("Revert transaction status not found")?;
            match status.status {
                TransactionStatus::FAILED => {
                    saw_failed = true;
                    break;
                }
                TransactionStatus::MINED | TransactionStatus::CONFIRMED => {
                    anyhow::bail!(
                        "Expected the transferFrom to revert on-chain, but it {:?}",
                        status.status
                    );
                }
                _ => tokio::time::sleep(Duration::from_millis(300)).await,
            }
        }
        anyhow::ensure!(saw_failed, "Reverting transaction never resolved FAILED");

        // The mined revert consumed the nonce - the follow-up must mine
        self.wait_for_transaction_completion(&follow_up_tx.id)
            .await
            .context("Follow-up transaction stuck behind mined revert")?;

        info!("[SUCCESS] On-chain revert resolved FAILED with no stranded nonce");
        Ok(())
    }
}
