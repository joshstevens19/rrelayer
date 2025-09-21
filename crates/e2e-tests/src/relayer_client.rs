use crate::test_config::E2ETestConfig;
use alloy::network::AnyTransactionReceipt;
use alloy::primitives::U256;
use anyhow::{anyhow, Context, Result};
use rrelayer_core::common_types::PagingResult;
use rrelayer_core::relayer::api::CreateRelayerResult;
use rrelayer_core::transaction::types::Transaction;
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext},
    relayer::types::RelayerId,
    transaction::api::get_transaction_status::RelayTransactionStatusResult,
    transaction::api::send_transaction::{RelayTransactionRequest, SendTransactionResult},
    transaction::api::types::TransactionSpeed,
    transaction::types::{TransactionData, TransactionId, TransactionValue},
};
use rrelayer_sdk::SDK;
use serde_json::Value;
use std::str::FromStr;
use tracing::info;

#[derive(Clone)]
pub struct RelayerClient {
    pub sdk: SDK,
}

impl RelayerClient {
    pub fn new(config: &E2ETestConfig) -> Self {
        // Get auth credentials from environment
        let username = std::env::var("RRELAYER_AUTH_USERNAME")
            .expect("RRELAYER_AUTH_USERNAME needs to be set");
        let password = std::env::var("RRELAYER_AUTH_PASSWORD")
            .expect("RRELAYER_AUTH_PASSWORD needs to be set");

        // Create SDK with auth
        let sdk = SDK::new(config.rrelayer_base_url.clone(), username, password);

        Self { sdk }
    }

    /// Create a new relayer for the test chain
    pub async fn create_relayer(&self, name: &str, chain_id: u64) -> Result<CreateRelayerResult> {
        info!("Creating relayer: {} on chain {}", name, chain_id);

        let result =
            self.sdk.relayer.create(chain_id, name).await.context("Failed to create relayer")?;

        info!("Created relayer: {:?}", result);

        Ok(result)
    }

    pub fn sent_transaction_compare(
        &self,
        sent: RelayTransactionRequest,
        transaction: Transaction,
        // _receipt: AnyTransactionReceipt, // TODO: add tests on the receipt as well
    ) -> Result<()> {
        // if transaction.is_noop {
        //     return Err(anyhow!("Transaction should not be an noop"));
        // }

        if transaction.to != sent.to {
            return Err(anyhow!(
                "Transaction to should be {} but it got sent to {}",
                transaction.to,
                sent.to
            ));
        }

        if transaction.value != sent.value {
            return Err(anyhow!(
                "Transaction value mismatch - expected {} but got {}",
                sent.value,
                transaction.value
            ));
        }

        if transaction.data != sent.data {
            return Err(anyhow!(
                "Transaction data mismatch - expected {} but got {}",
                sent.data,
                transaction.data
            ));
        }

        if transaction.external_id != sent.external_id {
            return Err(anyhow!(
                "Transaction external ids do not match expected {} but got {}",
                sent.external_id.expect("Should always be defined"),
                transaction.external_id.expect("Should always be defined"),
            ));
        }

        let transaction_blobs = transaction
            .blobs
            .as_ref()
            .map(|blobs| blobs.iter().map(|blob| blob.to_string()).collect::<Vec<String>>());

        if transaction_blobs != sent.blobs {
            return Err(anyhow!("Transaction blobs do not match expected",));
        }

        Ok(())
    }

    /// Send a transaction through the relayer
    pub async fn send_transaction(
        &self,
        relayer_id: &RelayerId,
        to: &EvmAddress,
        value: TransactionValue,
        data: TransactionData,
    ) -> Result<(SendTransactionResult, RelayTransactionRequest)> {
        info!("Sending transaction to: {} via relayer: {}", to, relayer_id);

        self.send_transaction_with_rate_limit_key(relayer_id, to, value, data, None).await
    }

    pub async fn send_transaction_with_rate_limit_key(
        &self,
        relayer_id: &RelayerId,
        to: &EvmAddress,
        value: TransactionValue,
        data: TransactionData,
        rate_limit_key: Option<String>,
    ) -> Result<(SendTransactionResult, RelayTransactionRequest)> {
        info!("Sending transaction to: {} via relayer: {}", to, relayer_id);

        let request = RelayTransactionRequest {
            to: to.clone(),
            value,
            data,
            speed: Some(TransactionSpeed::Fast),
            external_id: None,
            blobs: None,
        };

        info!("Transaction request: {:?}", request);

        let result = self
            .sdk
            .transaction
            .send_transaction(&relayer_id, &request, rate_limit_key)
            .await
            .context("Failed to send transaction")?;

        info!("Transaction response: {:?}", result);

        Ok((result, request))
    }

    /// Get transaction status
    pub async fn get_transaction_status(
        &self,
        transaction_id: &TransactionId,
    ) -> Result<RelayTransactionStatusResult> {
        info!("Getting transaction status for: {}", transaction_id);

        let result = self
            .sdk
            .transaction
            .get_transaction_status(&transaction_id)
            .await
            .context("Failed to get transaction status")?;

        let status_result = result.context("Transaction not found")?;

        info!("Transaction status: {:?}", status_result);

        Ok(status_result)
    }

    pub async fn get_transaction(&self, transaction_id: &TransactionId) -> Result<Transaction> {
        info!("Getting transaction status for: {}", transaction_id);

        let result = self
            .sdk
            .transaction
            .get_transaction(&transaction_id)
            .await
            .context("Failed to get transaction status")?;

        let tx = result.context("Transaction not found")?;

        info!("Transaction: {:?}", tx);

        Ok(tx)
    }
}
