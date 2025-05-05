use rrelayerr_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    relayer::types::RelayerId,
};

use crate::api::{http::HttpClient, types::ApiResult};

pub struct RelayerAllowlist {
    client: HttpClient,
}

impl RelayerAllowlist {
    pub(crate) fn new(client: HttpClient) -> Self {
        Self { client }
    }

    pub async fn add(&self, relayer_id: &RelayerId, address: &EvmAddress) -> ApiResult<()> {
        self.client.post_status(&format!("relayers/{}/allowlists/{}", relayer_id, address.hex()), &()).await
    }

    pub async fn delete(&self, relayer_id: &RelayerId, address: &EvmAddress) -> ApiResult<()> {
        self.client.delete_status(&format!("relayers/{}/allowlists/{}", relayer_id, address.hex())).await
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
