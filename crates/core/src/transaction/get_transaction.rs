use std::sync::Arc;

use super::{
    cache::{get_transaction_cache, set_transaction_cache},
    types::{Transaction, TransactionId},
};
use crate::{
    postgres::{PostgresClient, PostgresError},
    shared::cache::Cache,
};

/// Retrieves a transaction by its ID, first checking the cache then the database.
///
/// This function implements a cache-first strategy: it first attempts to retrieve
/// the transaction from the cache, and if not found, queries the database and
/// caches the result.
///
/// # Arguments
/// * `cache` - The cache instance to check for cached transactions
/// * `db` - The PostgreSQL database client to query if cache miss occurs
/// * `id` - The transaction ID to retrieve
///
/// # Returns
/// * `Ok(Some(Transaction))` - The transaction if found in cache or database
/// * `Ok(None)` - If the transaction doesn't exist in the database
/// * `Err(PostgresError)` - If a database error occurs
pub async fn get_transaction_by_id(
    cache: &Arc<Cache>,
    db: &PostgresClient,
    id: TransactionId,
) -> Result<Option<Transaction>, PostgresError> {
    if let Some(cached_transaction) = get_transaction_cache(cache, &id).await {
        return Ok(Some(cached_transaction));
    }

    let transaction = db.get_transaction(&id).await?;

    set_transaction_cache(cache, &id, &transaction).await;

    Ok(transaction)
}
