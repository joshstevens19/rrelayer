use std::sync::Arc;

use super::types::Network;
use crate::shared::cache::{Cache, CacheValue};

const NETWORKS_CACHE_KEY: &str = "networks";

/// Retrieves all networks from cache if available.
///
/// Attempts to fetch the cached networks data to avoid expensive database queries.
///
/// # Arguments
/// * `cache` - Shared cache instance containing network data
///
/// # Returns
/// * `Some(Vec<Network>)` - Cached networks if available
/// * `None` - If no cached data exists
pub async fn get_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

/// Stores networks data in cache for future retrieval.
///
/// Caches the provided networks to improve performance on subsequent requests.
///
/// # Arguments
/// * `cache` - Shared cache instance to store data in
/// * `networks` - Network data to be cached
pub async fn set_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache.insert(NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned())).await;
}

/// Invalidates the networks cache entry.
///
/// Removes cached networks data, forcing the next request to fetch from database.
///
/// # Arguments
/// * `cache` - Shared cache instance to invalidate
pub async fn invalidate_networks_cache(cache: &Arc<Cache>) {
    cache.delete(NETWORKS_CACHE_KEY).await;
}

const ENABLED_NETWORKS_CACHE_KEY: &str = "enabled_networks";

/// Retrieves enabled networks from cache if available.
///
/// Attempts to fetch cached enabled networks to avoid database queries.
///
/// # Arguments
/// * `cache` - Shared cache instance containing network data
///
/// # Returns
/// * `Some(Vec<Network>)` - Cached enabled networks if available
/// * `None` - If no cached data exists
pub async fn get_enabled_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(ENABLED_NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

/// Stores enabled networks data in cache for future retrieval.
///
/// Caches the provided enabled networks to improve performance on subsequent requests.
///
/// # Arguments
/// * `cache` - Shared cache instance to store data in
/// * `networks` - Enabled network data to be cached
pub async fn set_enabled_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache
        .insert(ENABLED_NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned()))
        .await;
}

/// Invalidates enabled networks cache and related cache entries.
///
/// Removes cached enabled networks data and also invalidates disabled networks
/// and general networks cache to maintain consistency.
///
/// # Arguments
/// * `cache` - Shared cache instance to invalidate
pub async fn invalidate_enabled_networks_cache(cache: &Arc<Cache>) {
    cache.delete(ENABLED_NETWORKS_CACHE_KEY).await;
    cache.delete(DISABLED_NETWORKS_CACHE_KEY).await;
    invalidate_networks_cache(cache).await;
}

const DISABLED_NETWORKS_CACHE_KEY: &str = "disabled_networks";

/// Retrieves disabled networks from cache if available.
///
/// Attempts to fetch cached disabled networks to avoid database queries.
///
/// # Arguments
/// * `cache` - Shared cache instance containing network data
///
/// # Returns
/// * `Some(Vec<Network>)` - Cached disabled networks if available
/// * `None` - If no cached data exists
pub async fn get_disabled_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(DISABLED_NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

/// Stores disabled networks data in cache for future retrieval.
///
/// Caches the provided disabled networks to improve performance on subsequent requests.
///
/// # Arguments
/// * `cache` - Shared cache instance to store data in
/// * `networks` - Disabled network data to be cached
pub async fn set_disabled_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache
        .insert(DISABLED_NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned()))
        .await;
}

/// Invalidates disabled networks cache and related cache entries.
///
/// Removes cached disabled networks data and also invalidates enabled networks
/// and general networks cache to maintain consistency.
///
/// # Arguments
/// * `cache` - Shared cache instance to invalidate
pub async fn invalidate_disabled_networks_cache(cache: &Arc<Cache>) {
    cache.delete(DISABLED_NETWORKS_CACHE_KEY).await;
    cache.delete(ENABLED_NETWORKS_CACHE_KEY).await;
    invalidate_networks_cache(cache).await;
}
