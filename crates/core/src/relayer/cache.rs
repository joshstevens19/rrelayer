use std::sync::Arc;

use super::types::{Relayer, RelayerId};
use crate::shared::cache::{Cache, CacheValue};

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
