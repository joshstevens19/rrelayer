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
