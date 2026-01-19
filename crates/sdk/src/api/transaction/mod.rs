use crate::ApiSdkError;
use crate::api::{http::HttpClient, types::ApiResult};
use reqwest::header::{HeaderMap, HeaderValue};
use rrelayer_core::transaction::api::{CancelTransactionResponse, RelayTransactionStatusResult};
use rrelayer_core::transaction::api::{RelayTransactionRequest, SendTransactionResult};
use rrelayer_core::transaction::queue_system::ReplaceTransactionResult;
use rrelayer_core::transaction::types::TransactionStatus;
use rrelayer_core::{
    RATE_LIMIT_HEADER_NAME,
    common_types::{PagingContext, PagingResult},
    relayer::RelayerId,
    transaction::types::{Transaction, TransactionHash, TransactionId},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TransactionApi {
    client: Arc<HttpClient>,
}

impl TransactionApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    pub async fn get(&self, transaction_id: &TransactionId) -> ApiResult<Option<Transaction>> {
        self.client.get_or_none(&format!("transactions/{}", transaction_id)).await
    }

    pub async fn get_by_tx_hash(
        &self,
        tx_hash: &TransactionHash,
    ) -> ApiResult<Option<Transaction>> {
        self.client.get_or_none(&format!("transactions/hash/{}", tx_hash)).await
    }

    pub async fn get_by_external_id(&self, external_id: &str) -> ApiResult<Option<Transaction>> {
        self.client.get_or_none(&format!("transactions/external/{}", external_id)).await
    }

    pub async fn get_all(
        &self,
        relayer_id: &RelayerId,
        paging: &PagingContext,
    ) -> ApiResult<PagingResult<Transaction>> {
        self.client
            .get_with_query(&format!("transactions/relayers/{}", relayer_id), Some(paging))
            .await
    }

    pub async fn send(
        &self,
        relayer_id: &RelayerId,
        transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SendTransactionResult> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }
        self.client
            .post_with_headers(
                &format!("transactions/relayers/{}/send", relayer_id),
                transaction,
                headers,
            )
            .await
    }

    pub async fn send_random(
        &self,
        chain_id: u64,
        transaction: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SendTransactionResult> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }
        self.client
            .post_with_headers(
                &format!("transactions/relayers/{}/send-random", chain_id),
                transaction,
                headers,
            )
            .await
    }

    pub async fn cancel(
        &self,
        transaction_id: &TransactionId,
        rate_limit_key: Option<String>,
    ) -> ApiResult<CancelTransactionResponse> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }

        self.client
            .put_with_headers(&format!("transactions/cancel/{}", transaction_id), &(), headers)
            .await
    }

    pub async fn replace(
        &self,
        transaction_id: &TransactionId,
        replacement: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> ApiResult<ReplaceTransactionResult> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }

        self.client
            .put_with_headers(
                &format!("transactions/replace/{}", transaction_id),
                replacement,
                headers,
            )
            .await
    }

    pub async fn get_status(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<RelayTransactionStatusResult>> {
        self.client.get_or_none(&format!("transactions/status/{}", transaction_id)).await
    }

    pub async fn get_inmempool_count(&self, relayer_id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/inmempool/count", relayer_id)).await
    }

    pub async fn get_pending_count(&self, relayer_id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/pending/count", relayer_id)).await
    }

    pub async fn wait_for_transaction_receipt_by_id(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<RelayTransactionStatusResult> {
        loop {
            let result = self.get_status(transaction_id).await?;
            if let Some(status_result) = result {
                match status_result.status {
                    TransactionStatus::PENDING | TransactionStatus::INMEMPOOL => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }
                    TransactionStatus::MINED
                    | TransactionStatus::CONFIRMED
                    | TransactionStatus::FAILED => {
                        return Ok(status_result);
                    }
                    TransactionStatus::EXPIRED => {
                        return Err(ApiSdkError::ConfigError("Transaction expired".to_string()));
                    }
                    TransactionStatus::CANCELLED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was cancelled".to_string(),
                        ));
                    }
                    TransactionStatus::REPLACED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was replaced".to_string(),
                        ));
                    }
                    TransactionStatus::DROPPED => {
                        return Err(ApiSdkError::ConfigError(
                            "Transaction was dropped from mempool".to_string(),
                        ));
                    }
                }
            } else {
                return Err(ApiSdkError::ConfigError("Transaction not found".to_string()));
            }
        }
    }
}
