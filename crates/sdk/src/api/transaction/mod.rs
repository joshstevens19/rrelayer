use crate::api::{http::HttpClient, types::ApiResult};
use reqwest::header::{HeaderMap, HeaderValue};
use rrelayer_core::network::ChainId;
use rrelayer_core::transaction::api::{CancelTransactionResponse, RelayTransactionStatusResult};
use rrelayer_core::transaction::api::{RelayTransactionRequest, SendTransactionResult};
use rrelayer_core::transaction::queue_system::ReplaceTransactionResult;
use rrelayer_core::{
    RATE_LIMIT_HEADER_NAME,
    common_types::{PagingContext, PagingResult},
    relayer::RelayerId,
    transaction::types::{Transaction, TransactionId},
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
        chain_id: &ChainId,
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
}
