use std::sync::Arc;

use super::types::{Transaction, TransactionId};
use crate::shared::cache::{Cache, CacheValue};

const TRANSACTION_CACHE_KEY: &str = "transaction";

/// Builds a cache key for a transaction using its ID.
///
/// # Arguments
/// * `id` - The transaction ID to build a cache key for
///
/// # Returns
/// * `String` - The formatted cache key
fn build_transaction_cache_key(id: &TransactionId) -> String {
    format!("{}__{}", TRANSACTION_CACHE_KEY, id)
}

/// Retrieves a transaction from the cache by its ID.
///
/// # Arguments
/// * `cache` - The cache instance to query
/// * `id` - The transaction ID to look up
///
/// # Returns
/// * `Some(Transaction)` - The cached transaction if found
/// * `None` - If the transaction is not in the cache
pub async fn get_transaction_cache(cache: &Arc<Cache>, id: &TransactionId) -> Option<Transaction> {
    if let Some(cached_result) = cache.get(&build_transaction_cache_key(id).to_string()).await {
        return cached_result.to_transaction();
    }

    None
}

/// Stores a transaction in the cache with the given ID.
///
/// # Arguments
/// * `cache` - The cache instance to store in
/// * `id` - The transaction ID to use as the cache key
/// * `transaction` - The transaction to cache (can be None)
pub async fn set_transaction_cache(
    cache: &Arc<Cache>,
    id: &TransactionId,
    transaction: &Option<Transaction>,
) {
    cache
        .insert(
            build_transaction_cache_key(id).to_string(),
            CacheValue::Transaction(transaction.clone()),
        )
        .await;
}

/// Removes a transaction from the cache by its ID.
///
/// # Arguments
/// * `cache` - The cache instance to remove from
/// * `id` - The transaction ID to remove from the cache
pub async fn invalidate_transaction_no_state_cache(cache: &Arc<Cache>, id: &TransactionId) {
    cache.delete(&build_transaction_cache_key(id).to_string()).await;
}
