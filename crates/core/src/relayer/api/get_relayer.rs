use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::shared::{not_found, HttpError};
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
pub async fn get_relayer_api(
    State(state): State<Arc<AppState>>,
    Path(relayer_id): Path<RelayerId>,
    headers: HeaderMap,
) -> Result<Json<GetRelayerResult>, HttpError> {
    state.validate_allowed_passed_basic_auth(&headers)?;

    let relayer = get_relayer(&state.db, &state.cache, &relayer_id)
        .await?
        .ok_or(not_found("Relayer could not be found".to_string()))?;

    state.validate_auth_basic_or_api_key(&headers, &relayer.address, &relayer.chain_id)?;

    let provider = find_provider_for_chain_id(&state.evm_providers, &relayer.chain_id).await;
    let provider_urls = provider.map(|p| p.provider_urls.clone()).unwrap_or_default();

    Ok(Json(GetRelayerResult { relayer, provider_urls }))
}
