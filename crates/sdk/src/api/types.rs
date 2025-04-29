use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum ApiBaseConfig {
    WithAuthToken { server_url: String, auth_token: String },
    WithApiKey { server_url: String, api_key: String },
    Basic { server_url: String },
}

#[derive(Error, Debug)]
pub enum ApiSdkError {
    #[error("HTTP client error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type ApiResult<T> = Result<T, ApiSdkError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagingContext {
    pub limit: u32,
    pub offset: u32,
}

impl PagingContext {
    pub fn new(limit: u32, offset: u32) -> Self {
        Self { limit, offset }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagingResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}
