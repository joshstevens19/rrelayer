use std::sync::Arc;

use super::types::Network;
use crate::shared::cache::{Cache, CacheValue};

const NETWORKS_CACHE_KEY: &str = "networks";

pub async fn get_networks_cache(cache: &Arc<Cache>) -> Vec<Network> {
    if let Some(cached_result) = cache.get(NETWORKS_CACHE_KEY).await {
        return cached_result.to_networks();
    }

    vec![]
}

pub async fn set_networks_cache(cache: &Arc<Cache>, networks: &[Network]) {
    cache.insert(NETWORKS_CACHE_KEY.to_string(), CacheValue::Networks(networks.to_owned())).await;
}
