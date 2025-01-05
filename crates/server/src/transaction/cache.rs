use std::sync::Arc;

use super::types::{Transaction, TransactionId};
use crate::shared::cache::{Cache, CacheValue};

const TRANSACTION_CACHE_KEY: &str = "transaction";

fn build_transaction_cache_key(id: &TransactionId) -> String {
    format!("{}__{}", TRANSACTION_CACHE_KEY, id)
}

pub async fn get_transaction_cache(cache: &Arc<Cache>, id: &TransactionId) -> Option<Transaction> {
    if let Some(cached_result) = cache.get(&build_transaction_cache_key(id).to_string()).await {
        return cached_result.to_transaction();
    }

    None
}

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

pub async fn invalidate_transaction_no_state_cache(cache: &Arc<Cache>, id: &TransactionId) {
    cache.delete(&build_transaction_cache_key(id).to_string()).await;
}
