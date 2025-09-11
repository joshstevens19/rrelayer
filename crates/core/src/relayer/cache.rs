use std::sync::Arc;

use super::types::{Relayer, RelayerId};
use crate::shared::cache::{Cache, CacheValue};

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
