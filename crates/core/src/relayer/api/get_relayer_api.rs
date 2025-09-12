use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    app_state::AppState,
    provider::find_provider_for_chain_id,
    relayer::{
        get_relayer,
        types::{Relayer, RelayerId},
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GetRelayerResult {
    pub relayer: Relayer,
    #[serde(rename = "providerUrls")]
    pub provider_urls: Vec<String>,
}

/// Retrieves detailed information about a specific relayer.
///
/// This endpoint returns relayer details including its configuration and associated
/// provider URLs.
///
/// # Arguments
/// * `state` - Application state containing database and provider connections
/// * `auth_guard` - Authentication guard that validates basic auth
/// * `relayer_id` - The unique identifier of the relayer to retrieve
///
/// # Returns
/// * `Ok(Json<GetRelayerResult>)` - Relayer details and provider URLs
/// * `Err(StatusCode)` - HTTP error code if retrieval fails or unauthorized
pub async fn get_relayer_api(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
) -> Result<Json<GetRelayerResult>, StatusCode> {
    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id).await;
    let provider_urls = provider.map(|p| p.provider_urls.clone()).unwrap_or_default();

    Ok(Json(GetRelayerResult { relayer, provider_urls }))
}
