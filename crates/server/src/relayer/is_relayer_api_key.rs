use std::sync::Arc;

use axum::http::HeaderMap;

use super::{
    cache::{get_is_relayer_api_key_cache, set_is_relayer_api_key_cache},
    types::RelayerId,
};
use crate::{
    postgres::PostgresClient,
    shared::{cache::Cache, common_types::api_key_from_headers},
};

pub async fn is_relayer_api_key(
    db: &PostgresClient,
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
    headers: &HeaderMap,
) -> bool {
    let api_key_result = api_key_from_headers(headers);

    match api_key_result {
        None => false,
        Some(api_key) => {
            if let Some(cached_result) =
                get_is_relayer_api_key_cache(cache, relayer_id, &api_key).await
            {
                return cached_result;
            }

            let result = db.is_relayer_api_key(relayer_id, &api_key).await;
            match result {
                Ok(result) => {
                    set_is_relayer_api_key_cache(cache, relayer_id, &api_key, &result).await;
                    result
                }
                Err(_) => false,
            }
        }
    }
}
