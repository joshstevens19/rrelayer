use crate::api::{http::HttpClient, types::ApiResult};
use rrelayer_core::transaction::api::get_transaction_status::RelayTransactionStatusResult;
use rrelayer_core::transaction::api::send_transaction::{
    RelayTransactionRequest, SendTransactionResult,
};
use rrelayer_core::{
    common_types::{PagingContext, PagingResult},
    relayer::types::RelayerId,
    transaction::types::{Transaction, TransactionId},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct TransactionApi {
    client: Arc<HttpClient>,
}

impl TransactionApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    pub async fn get_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<Transaction>> {
        self.client.get(&format!("transactions/{}", transaction_id)).await
    }

    pub async fn get_transactions(
        &self,
        relayer_id: &RelayerId,
        paging: &PagingContext,
    ) -> ApiResult<PagingResult<Transaction>> {
        self.client
            .get_with_query(&format!("transactions/relayers/{}", relayer_id), Some(paging))
            .await
    }

    pub async fn send_transaction(
        &self,
        relayer_id: &RelayerId,
        transaction: &RelayTransactionRequest,
    ) -> ApiResult<SendTransactionResult> {
        self.client.post(&format!("transactions/relayers/{}/send", relayer_id), transaction).await
    }

    pub async fn cancel_transaction(&self, transaction_id: &TransactionId) -> ApiResult<bool> {
        self.client.put(&format!("transactions/cancel/{}", transaction_id), &()).await
    }

    pub async fn replace_transaction(
        &self,
        transaction_id: &TransactionId,
        replacement: &RelayTransactionRequest,
    ) -> ApiResult<bool> {
        self.client.put(&format!("transactions/replace/{}", transaction_id), replacement).await
    }

    pub async fn get_transaction_status(
        &self,
        transaction_id: &TransactionId,
    ) -> ApiResult<Option<RelayTransactionStatusResult>> {
        self.client.get(&format!("transactions/status/{}", transaction_id)).await
    }

    pub async fn get_transactions_inmempool_count(&self, relayer_id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/inmempool/count", relayer_id)).await
    }

    pub async fn get_transactions_pending_count(&self, relayer_id: &RelayerId) -> ApiResult<u32> {
        self.client.get(&format!("transactions/relayers/{}/pending/count", relayer_id)).await
    }
}
