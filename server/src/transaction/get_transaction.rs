use std::sync::Arc;

use super::{
    cache::{get_transaction_cache, set_transaction_cache},
    types::{Transaction, TransactionId},
};
use crate::{postgres::PostgresClient, shared::cache::Cache};

pub async fn get_transaction_by_id(
    cache: &Arc<Cache>,
    db: &PostgresClient,
    id: TransactionId,
) -> Result<Option<Transaction>, tokio_postgres::Error> {
    if let Some(cached_transaction) = get_transaction_cache(cache, &id).await {
        return Ok(Some(cached_transaction));
    }

    let transaction = db.get_transaction(&id).await?;

    set_transaction_cache(cache, &id, &transaction).await;

    Ok(transaction)
}
