use anyhow::{Context, Result};
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

use crate::test_config::E2ETestConfig;

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
    pub async fn create_relayer(&self, name: &str, chain_id: u64) -> Result<Value> {
        info!("Creating relayer: {} on chain {}", name, chain_id);

        let result =
            self.sdk.relayer.create(chain_id, name).await.context("Failed to create relayer")?;

        info!("Created relayer: {:?}", result);

        // Convert to serde_json::Value for compatibility
        let relayer: Value = serde_json::to_value(result)?;
        Ok(relayer)
    }

    /// Send a transaction through the relayer
    pub async fn send_transaction(
        &self,
        relayer_id: &RelayerId,
        to: &str,
        value: Option<&str>,
        data: Option<&str>,
    ) -> Result<SendTransactionResult> {
        info!("Sending transaction to: {} via relayer: {}", to, relayer_id);

        // Parse the inputs to the proper types
        let to_address = EvmAddress::from_str(to).context("Invalid to address")?;

        let transaction_value = match value {
            Some(v) => TransactionValue::from_str(v)
                .map_err(|e| anyhow::anyhow!("Invalid value: {}", e))?,
            None => TransactionValue::default(),
        };

        let transaction_data = match data {
            Some(d) => {
                TransactionData::from_str(d).map_err(|e| anyhow::anyhow!("Invalid data: {}", e))?
            }
            None => TransactionData::default(),
        };

        let request = RelayTransactionRequest {
            to: to_address,
            value: transaction_value,
            data: transaction_data,
            speed: Some(TransactionSpeed::Fast),
            external_id: None,
            blobs: None,
        };

        info!("Transaction request: {:?}", request);

        let result = self
            .sdk
            .transaction
            .send_transaction(&relayer_id, &request)
            .await
            .context("Failed to send transaction")?;

        info!("Transaction response: {:?}", result);

        Ok(result)
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

    /// Get relayer transactions with pagination
    pub async fn get_relayer_transactions(
        &self,
        relayer_id: &RelayerId,
        limit: u32,
        offset: u32,
    ) -> Result<Value> {
        info!(
            "Getting relayer transactions for: {} (limit: {}, offset: {})",
            relayer_id, limit, offset
        );
        let paging = PagingContext { limit, offset };

        let result = self
            .sdk
            .transaction
            .get_transactions(&relayer_id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!("Relayer transactions: {:?}", result);

        let transactions: Value = serde_json::to_value(result)?;
        Ok(transactions)
    }

    /// Get pending transaction count for a relayer
    pub async fn get_pending_count(&self, relayer_id: &RelayerId) -> Result<u64> {
        info!("Getting pending count for relayer: {}", relayer_id);

        let count = self
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer_id)
            .await
            .context("Failed to get pending count")?;

        info!("Pending count: {}", count);

        Ok(count as u64)
    }
}
