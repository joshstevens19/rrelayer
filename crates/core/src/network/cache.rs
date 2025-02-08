use std::sync::Arc;

use super::types::Network;
use crate::shared::cache::{Cache, CacheValue};

const NETWORKS_CACHE_KEY: &str = "networks";

pub async fn get_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

pub async fn set_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache.insert(NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned())).await;
}

pub async fn invalidate_networks_cache(cache: &Arc<Cache>) {
    cache.delete(NETWORKS_CACHE_KEY).await;
}

const ENABLED_NETWORKS_CACHE_KEY: &str = "enabled_networks";

pub async fn get_enabled_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(ENABLED_NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

pub async fn set_enabled_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache
        .insert(ENABLED_NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned()))
        .await;
}

pub async fn invalidate_enabled_networks_cache(cache: &Arc<Cache>) {
    cache.delete(ENABLED_NETWORKS_CACHE_KEY).await;
    cache.delete(DISABLED_NETWORKS_CACHE_KEY).await;
    invalidate_networks_cache(cache).await;
}

const DISABLED_NETWORKS_CACHE_KEY: &str = "disabled_networks";

pub async fn get_disabled_networks_cache(cache: &Arc<Cache>) -> Option<Vec<Network>> {
    if let Some(cached_result) = cache.get(DISABLED_NETWORKS_CACHE_KEY).await {
        return Some(cached_result.to_networks());
    }

    None
}

pub async fn set_disabled_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache
        .insert(DISABLED_NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned()))
        .await;
}

pub async fn invalidate_disabled_networks_cache(cache: &Arc<Cache>) {
    cache.delete(DISABLED_NETWORKS_CACHE_KEY).await;
    cache.delete(ENABLED_NETWORKS_CACHE_KEY).await;
    invalidate_networks_cache(cache).await;
}
