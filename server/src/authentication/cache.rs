use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use crate::shared::{
    cache::{Cache, CacheValue},
    common_types::EvmAddress,
};

const AUTHENTICATION_CACHE_KEY: &str = "authentication";

fn build_authentication_cache_key(id: &Uuid, address: &EvmAddress) -> String {
    format!("{}__{}__{}", AUTHENTICATION_CACHE_KEY, id, address)
}

pub async fn get_authentication_challenge_cache(
    cache: &Arc<Cache>,
    id: &Uuid,
    address: &EvmAddress,
) -> Option<String> {
    if let Some(cached_result) =
        cache.get(&build_authentication_cache_key(id, address).to_string()).await
    {
        return Some(cached_result.to_authentication_challenge());
    }

    None
}

pub async fn set_authentication_challenge_cache(
    cache: &Arc<Cache>,
    id: &Uuid,
    address: &EvmAddress,
    challenge: &str,
) {
    cache
        .insert_with_expiry(
            build_authentication_cache_key(id, address).to_string(),
            CacheValue::AuthenticationChallenge(challenge.to_string()),
            // 5 minutes its valid for the challenge
            Duration::from_secs(300 * 10),
        )
        .await;
}

pub async fn invalidate_authentication_challenge_cache(
    cache: &Arc<Cache>,
    id: &Uuid,
    address: &EvmAddress,
) {
    cache.delete(&build_authentication_cache_key(id, address).to_string()).await;
}
