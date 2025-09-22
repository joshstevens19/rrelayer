use thiserror::Error;

use crate::commands::error::{
    AllowlistError, ApiKeyError, BalanceError, ConfigError, InitError, NetworkError,
    ProjectLocationError, ProjectStartupError, RelayerManagementError, SigningError,
    TransactionError, UserError,
};

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Project location error: {0}")]
    ProjectLocation(#[from] ProjectLocationError),

    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    #[error("Balance error: {0}")]
    Balance(#[from] BalanceError),

    #[error("Signing error: {0}")]
    Signing(#[from] SigningError),

    #[error("API key error: {0}")]
    ApiKey(#[from] ApiKeyError),

    #[error("User error: {0}")]
    User(#[from] UserError),

    #[error("Allowlist error: {0}")]
    Allowlist(#[from] AllowlistError),

    #[error("Initialization error: {0}")]
    Init(#[from] InitError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Project startup error: {0}")]
    ProjectStartup(#[from] ProjectStartupError),

    #[error("Relayer management error: {0}")]
    RelayerManagement(#[from] RelayerManagementError),

    #[error("Core startup error: {0}")]
    CoreStartup(#[from] rrelayer_core::StartError),

    #[error("Core database connection error: {0}")]
    CoreDatabase(#[from] rrelayer_core::PostgresConnectionError),

    #[error("Wallet operation error: {0}")]
    WalletError(#[from] rrelayer_core::WalletError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("String encoding error: {0}")]
    StringEncoding(#[from] std::string::FromUtf8Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Core write error: {0}")]
    CoreWrite(#[from] rrelayer_core::WriteFileError),

    #[error("Terminal interaction error: {0}")]
    Terminal(#[from] dialoguer::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<Box<dyn std::error::Error>> for CliError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        CliError::Internal(err.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CliError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        CliError::Internal(err.to_string())
    }
}

impl From<String> for CliError {
    fn from(err: String) -> Self {
        CliError::Internal(err)
    }
}

impl From<&str> for CliError {
    fn from(err: &str) -> Self {
        CliError::Internal(err.to_string())
    }
}
