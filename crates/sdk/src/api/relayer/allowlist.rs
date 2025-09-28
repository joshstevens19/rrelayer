use rrelayer_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    relayer::RelayerId,
};
use std::sync::Arc;

use crate::api::{http::HttpClient, types::ApiResult};

#[derive(Debug, Clone)]
pub struct RelayerAllowlist {
    client: Arc<HttpClient>,
}

impl RelayerAllowlist {
    pub(crate) fn new(client: Arc<HttpClient>) -> Self {
        Self { client }
    }

    pub async fn get_all(
        &self,
        relayer_id: &RelayerId,
        paging: &PagingContext,
    ) -> ApiResult<PagingResult<EvmAddress>> {
        self.client
            .get_with_query(&format!("relayers/{}/allowlists", relayer_id), Some(paging))
            .await
    }
}
