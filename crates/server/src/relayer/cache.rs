use std::sync::Arc;

use axum::extract::State;

use super::types::{Relayer, RelayerId};
use crate::shared::{
    cache::{Cache, CacheValue},
    common_types::ApiKey,
};

const RELAYER_CACHE_KEY: &str = "relayer";

fn build_relayer_cache_key(id: &RelayerId) -> String {
    format!("{}__{}", RELAYER_CACHE_KEY, id)
}

pub async fn get_relayer_cache(cache: &Arc<Cache>, relayer_id: &RelayerId) -> Option<Relayer> {
    if let Some(cached_result) = cache.get(&build_relayer_cache_key(relayer_id).to_string()).await {
        return cached_result.to_relayer();
    }

    None
}

pub async fn set_relayer_cache(
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
    relayer: &Option<Relayer>,
) {
    cache
        .insert(
            build_relayer_cache_key(relayer_id).to_string(),
            CacheValue::Relayer(relayer.clone()),
        )
        .await;
}

pub async fn invalidate_relayer_cache(cache: &Arc<Cache>, relayer_id: &RelayerId) {
    cache.delete(&build_relayer_cache_key(relayer_id).to_string()).await;
}

const IS_RELAYER_API_KEY_CACHE_KEY: &str = "relayer_api_key";

fn build_is_relayer_api_key_cache_key(id: &RelayerId, api_key: &ApiKey) -> String {
    format!("{}__{}__{}", IS_RELAYER_API_KEY_CACHE_KEY, id, api_key)
}

pub async fn get_is_relayer_api_key_cache(
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
    api_key: &ApiKey,
) -> Option<bool> {
    if let Some(cached_result) =
        cache.get(&build_is_relayer_api_key_cache_key(relayer_id, api_key).to_string()).await
    {
        return Some(cached_result.to_is_relayer_api_key());
    }

    None
}

pub async fn set_is_relayer_api_key_cache(
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
    api_key: &ApiKey,
    result: &bool,
) {
    cache
        .insert(
            build_is_relayer_api_key_cache_key(relayer_id, api_key).to_string(),
            CacheValue::IsRelayerApiKey(*result),
        )
        .await;
}

pub async fn invalidate_is_relayer_api_key_cache(
    cache: &State<Arc<Cache>>,
    api_key: &ApiKey,
    relayer_id: &RelayerId,
) {
    cache.delete(&build_is_relayer_api_key_cache_key(relayer_id, api_key).to_string()).await;
}
