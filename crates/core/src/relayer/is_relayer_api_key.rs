use std::sync::Arc;

use axum::http::HeaderMap;

use super::{
    cache::{get_is_relayer_api_key_cache, set_is_relayer_api_key_cache},
    types::RelayerId,
};
use crate::{
    postgres::PostgresClient,
    shared::{cache::Cache, common_types::api_key_from_headers},
};

/// Validates if the provided API key is valid for the specified relayer.
///
/// This function extracts the API key from the HTTP headers and checks if it's
/// a valid API key for the given relayer. It uses caching to avoid repeated
/// database queries for the same API key validations.
///
/// # Arguments
/// * `db` - The PostgreSQL client for database operations
/// * `cache` - The cache instance for storing/retrieving validation results
/// * `relayer_id` - The unique identifier of the relayer
/// * `headers` - HTTP headers containing the API key
///
/// # Returns
/// * `true` - If a valid API key is found in headers and it belongs to the relayer
/// * `false` - If no API key is provided, the API key is invalid, or database error occurs
pub async fn is_relayer_api_key(
    db: &PostgresClient,
    cache: &Arc<Cache>,
    relayer_id: &RelayerId,
    headers: &HeaderMap,
) -> bool {
    let api_key_result = api_key_from_headers(headers);

    match api_key_result {
        None => false,
        Some(api_key) => {
            if let Some(cached_result) =
                get_is_relayer_api_key_cache(cache, relayer_id, &api_key).await
            {
                return cached_result;
            }

            let result = db.is_relayer_api_key(relayer_id, &api_key).await;
            match result {
                Ok(result) => {
                    set_is_relayer_api_key_cache(cache, relayer_id, &api_key, &result).await;
                    result
                }
                Err(_) => false,
            }
        }
    }
}
