use crate::app_state::AppState;
use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::shared::HttpError;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub enum AuthType {
    BASIC,
    APIKEY,
}

impl fmt::Display for AuthType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthType::BASIC => write!(f, "BASIC"),
            AuthType::APIKEY => write!(f, "APIKEY"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyAccess {
    pub chain_id: ChainId,
    pub relayers: Vec<EvmAddress>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub authenticated_with: AuthType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_access: Option<Vec<ApiKeyAccess>>,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<StatusResponse>, HttpError> {
    let basic = state.validate_basic_auth_valid(&headers);
    match basic {
        Ok(_) => {
            return Ok(Json(StatusResponse {
                authenticated_with: AuthType::BASIC,
                api_key_access: None,
            }));
        }
        Err(_) => {
            let api_key_access = state.get_api_key_access(&headers);
            if api_key_access.is_some() {
                return Ok(Json(StatusResponse {
                    authenticated_with: AuthType::APIKEY,
                    api_key_access,
                }));
            }
        }
    }

    Err(crate::shared::unauthorized(None))
}
