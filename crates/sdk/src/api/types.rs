use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApiBaseConfig {
    pub server_url: String,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone)]
pub enum AuthConfig {
    BasicAuth { username: String, password: String },
    ApiKey { api_key: String },
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
