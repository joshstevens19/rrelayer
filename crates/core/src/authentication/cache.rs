use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use crate::shared::{
    cache::{Cache, CacheValue},
    common_types::EvmAddress,
};

const AUTHENTICATION_CACHE_KEY: &str = "authentication";

/// Builds a cache key for authentication challenges.
///
/// Creates a unique cache key by combining the base authentication cache key
/// with the provided UUID and EVM address.
///
/// # Arguments
/// * `id` - The unique identifier for the authentication challenge
/// * `address` - The EVM address associated with the authentication challenge
///
/// # Returns
/// * `String` - A formatted cache key string
fn build_authentication_cache_key(id: &Uuid, address: &EvmAddress) -> String {
    format!("{}__{}__{}", AUTHENTICATION_CACHE_KEY, id, address)
}

/// Retrieves an authentication challenge from the cache.
///
/// Attempts to find a cached authentication challenge using the provided
/// UUID and EVM address. The challenge is used for wallet signature verification.
///
/// # Arguments
/// * `cache` - The shared cache instance
/// * `id` - The unique identifier for the authentication challenge
/// * `address` - The EVM address associated with the authentication challenge
///
/// # Returns
/// * `Some(String)` - The cached authentication challenge if found
/// * `None` - If no challenge is found in the cache
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

/// Stores an authentication challenge in the cache with expiry.
///
/// Caches the authentication challenge string for a specific UUID and EVM address
/// combination. The challenge expires after 50 minutes (300 * 10 seconds).
///
/// # Arguments
/// * `cache` - The shared cache instance
/// * `id` - The unique identifier for the authentication challenge
/// * `address` - The EVM address associated with the authentication challenge
/// * `challenge` - The challenge string to be cached
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
            // 5 minutes it's valid for the challenge
            Duration::from_secs(300 * 10),
        )
        .await;
}

/// Removes an authentication challenge from the cache.
///
/// Deletes the cached authentication challenge for the given UUID and EVM address
/// combination. This is typically called after successful authentication to prevent
/// challenge reuse.
///
/// # Arguments
/// * `cache` - The shared cache instance
/// * `id` - The unique identifier for the authentication challenge
/// * `address` - The EVM address associated with the authentication challenge
pub async fn invalidate_authentication_challenge_cache(
    cache: &Arc<Cache>,
    id: &Uuid,
    address: &EvmAddress,
) {
    cache.delete(&build_authentication_cache_key(id, address).to_string()).await;
}
