use std::sync::Arc;

use super::{
    cache::{get_relayer_cache, set_relayer_cache},
    types::{Relayer, RelayerId, RelayerProviderContext},
};
use crate::{
    postgres::{PostgresClient, PostgresError},
    provider::{find_provider_for_chain_id, EvmProvider},
    shared::cache::Cache,
};

/// Retrieves a relayer by ID, using cache when available.
///
/// This function first checks the cache for the relayer data. If not found in cache,
/// it queries the database and then caches the result for future requests.
///
/// # Arguments
/// * `db` - The PostgreSQL client for database operations
/// * `cache` - The cache instance for storing/retrieving relayer data
/// * `relayer_id` - The unique identifier of the relayer to retrieve
///
/// # Returns
/// * `Ok(Some(Relayer))` - If the relayer is found
/// * `Ok(None)` - If the relayer doesn't exist
/// * `Err(PostgresError)` - If a database error occurs
pub async fn get_relayer(
    db: &PostgresClient,
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
) -> Result<Option<Relayer>, PostgresError> {
    if let Some(cached_result) = get_relayer_cache(cache, relayer_id).await {
        return Ok(Some(cached_result));
    }

    let relayer = db.get_relayer(relayer_id).await?;

    set_relayer_cache(cache, relayer_id, &relayer).await;

    Ok(relayer)
}

pub async fn relayer_exists(
    db: &PostgresClient,
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
) -> Result<bool, PostgresError> {
    let relayer = get_relayer(db, cache, relayer_id).await?;
    Ok(relayer.is_some())
}

/// Retrieves a relayer with its associated EVM provider context.
///
/// This function gets a relayer by ID and pairs it with the appropriate EVM provider
/// for the relayer's chain. The provider is found from the list of available providers
/// based on the relayer's chain ID.
///
/// # Arguments
/// * `db` - The PostgreSQL client for database operations
/// * `cache` - The cache instance for storing/retrieving relayer data
/// * `providers` - A list of available EVM providers
/// * `relayer_id` - The unique identifier of the relayer to retrieve
///
/// # Returns
/// * `Ok(Some(RelayerProviderContext))` - If the relayer and matching provider are found
/// * `Ok(None)` - If the relayer doesn't exist or no provider is found for its chain
/// * `Err(PostgresError)` - If a database error occurs
pub async fn get_relayer_provider_context_by_relayer_id<'a>(
    db: &Arc<PostgresClient>,
    cache: &Arc<Cache>,
    providers: &'a Vec<EvmProvider>,
    relayer_id: &RelayerId,
) -> Result<Option<RelayerProviderContext<'a>>, PostgresError> {
    let relayer = get_relayer(db, cache, relayer_id).await?;

    match relayer {
        Some(relayer) => {
            let provider = find_provider_for_chain_id(providers, &relayer.chain_id).await;
            match provider {
                Some(provider) => Ok(Some(RelayerProviderContext { relayer, provider })),
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}
