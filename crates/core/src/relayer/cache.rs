use std::sync::Arc;

use axum::extract::State;

use super::types::{Relayer, RelayerId};
use crate::shared::{
    cache::{Cache, CacheValue},
    common_types::ApiKey,
};

const RELAYER_CACHE_KEY: &str = "relayer";

/// Builds a cache key for relayer data.
///
/// # Arguments
/// * `id` - The relayer ID to include in the cache key
///
/// # Returns
/// * A formatted string that can be used as a cache key
fn build_relayer_cache_key(id: &RelayerId) -> String {
    format!("{}__{}", RELAYER_CACHE_KEY, id)
}

/// Retrieves a relayer from the cache.
///
/// # Arguments
/// * `cache` - The cache instance to query
/// * `relayer_id` - The ID of the relayer to retrieve
///
/// # Returns
/// * `Some(Relayer)` - If the relayer is found in cache
/// * `None` - If the relayer is not cached or cache lookup fails
pub async fn get_relayer_cache(cache: &Arc<Cache>, relayer_id: &RelayerId) -> Option<Relayer> {
    if let Some(cached_result) = cache.get(&build_relayer_cache_key(relayer_id).to_string()).await {
        return cached_result.to_relayer();
    }

    None
}

/// Stores a relayer in the cache.
///
/// # Arguments
/// * `cache` - The cache instance to store in
/// * `relayer_id` - The ID of the relayer to cache
/// * `relayer` - The relayer data to store (optional)
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

/// Removes a relayer from the cache.
///
/// # Arguments
/// * `cache` - The cache instance to remove from
/// * `relayer_id` - The ID of the relayer to remove from cache
pub async fn invalidate_relayer_cache(cache: &Arc<Cache>, relayer_id: &RelayerId) {
    cache.delete(&build_relayer_cache_key(relayer_id).to_string()).await;
}

const IS_RELAYER_API_KEY_CACHE_KEY: &str = "relayer_api_key";

/// Builds a cache key for relayer API key validation data.
///
/// # Arguments
/// * `id` - The relayer ID
/// * `api_key` - The API key to include in the cache key
///
/// # Returns
/// * A formatted string that can be used as a cache key
fn build_is_relayer_api_key_cache_key(id: &RelayerId, api_key: &ApiKey) -> String {
    format!("{}__{}__{}", IS_RELAYER_API_KEY_CACHE_KEY, id, api_key)
}

/// Retrieves the cached API key validation result for a relayer.
///
/// # Arguments
/// * `cache` - The cache instance to query
/// * `relayer_id` - The ID of the relayer
/// * `api_key` - The API key to check
///
/// # Returns
/// * `Some(bool)` - The cached validation result (true if valid, false if invalid)
/// * `None` - If the validation result is not cached
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

/// Stores the API key validation result for a relayer in the cache.
///
/// # Arguments
/// * `cache` - The cache instance to store in
/// * `relayer_id` - The ID of the relayer
/// * `api_key` - The API key that was validated
/// * `result` - The validation result to cache
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

/// Removes the API key validation result for a relayer from the cache.
///
/// # Arguments
/// * `cache` - The cache instance to remove from
/// * `api_key` - The API key to invalidate
/// * `relayer_id` - The ID of the relayer
pub async fn invalidate_is_relayer_api_key_cache(
    cache: &State<Arc<Cache>>,
    api_key: &ApiKey,
    relayer_id: &RelayerId,
) {
    cache.delete(&build_is_relayer_api_key_cache_key(relayer_id, api_key).to_string()).await;
}
