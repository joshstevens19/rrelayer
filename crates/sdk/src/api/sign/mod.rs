use crate::api::{http::HttpClient, types::ApiResult};
use reqwest::header::{HeaderMap, HeaderValue};
use rrelayer_core::RATE_LIMIT_HEADER_NAME;
use rrelayer_core::common_types::{PagingContext, PagingResult};
use rrelayer_core::relayer::RelayerId;
use rrelayer_core::signing::api::{SignTextResult, SignTypedDataResult};
use rrelayer_core::signing::db::read::{SignedTextHistory, SignedTypedDataHistory};
use std::sync::Arc;

#[derive(Clone)]
pub struct SignApi {
    client: Arc<HttpClient>,
}

impl SignApi {
    pub fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    pub async fn sign_text(
        &self,
        relayer_id: &RelayerId,
        text: &str,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SignTextResult> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }

        self.client
            .post_with_headers(
                &format!("signing/{}/message", relayer_id),
                &serde_json::json!({ "text": text }),
                headers,
            )
            .await
    }

    pub async fn sign_typed_data(
        &self,
        relayer_id: &RelayerId,
        typed_data: &alloy::dyn_abi::TypedData,
        rate_limit_key: Option<String>,
    ) -> ApiResult<SignTypedDataResult> {
        let mut headers = HeaderMap::new();
        if let Some(rate_limit_key) = rate_limit_key.as_ref() {
            headers.insert(
                RATE_LIMIT_HEADER_NAME,
                HeaderValue::from_str(rate_limit_key).expect("Invalid rate limit key"),
            );
        }

        self.client
            .post_with_headers(&format!("signing/{}/typed-data", relayer_id), typed_data, headers)
            .await
    }

    /// Retrieves the signing text history for a specific relayer with pagination.
    ///
    /// # Arguments
    /// * `relayer_id` - The relayer ID to get history for
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<SignedTextHistory>)` - Paginated list of signed text messages
    /// * `Err(ApiSdkError)` - If the API call fails
    pub async fn get_text_history(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<SignedTextHistory>> {
        let url = format!(
            "signing/{}/text-history?limit={}&offset={}",
            relayer_id, paging_context.limit, paging_context.offset
        );
        self.client.get(&url).await
    }

    /// Retrieves the signing typed data history for a specific relayer with pagination.
    ///
    /// # Arguments
    /// * `relayer_id` - The relayer ID to get history for
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<SignedTypedDataHistory>)` - Paginated list of signed typed data messages
    /// * `Err(ApiSdkError)` - If the API call fails
    pub async fn get_typed_data_history(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> ApiResult<PagingResult<SignedTypedDataHistory>> {
        let url = format!(
            "signing/{}/typed-data-history?limit={}&offset={}",
            relayer_id, paging_context.limit, paging_context.offset
        );
        self.client.get(&url).await
    }
}
