use thiserror::Error;

/// Errors that can occur during authentication operations
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid password or keystore not found")]
    InvalidCredentials,

    #[error("Password manager error: {0}")]
    PasswordManager(#[from] rrelayer_core::keystore::PasswordError),

    #[error("Keystore operation failed: {0}")]
    KeystoreOperation(#[from] rrelayer_core::WalletError),

    #[error("Terminal interaction failed: {0}")]
    Terminal(#[from] dialoguer::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Keystore error: {0}")]
    Keystore(#[from] KeystoreError),
}

/// Errors that can occur during keystore operations  
#[derive(Error, Debug)]
pub enum KeystoreError {
    #[error("Keystore already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid mnemonic phrase")]
    InvalidMnemonic,

    #[error("Invalid private key")]
    InvalidPrivateKey,

    #[error("Keystore not found: {0}")]
    NotFound(String),

    #[error("Failed to decrypt keystore: {0}")]
    DecryptionFailed(String),

    #[error("Project configuration error: {0}")]
    ProjectConfig(String),

    #[error("Wallet operation failed: {0}")]
    WalletOperation(#[from] rrelayer_core::WalletError),

    #[error("Password manager error: {0}")]
    PasswordManager(#[from] rrelayer_core::keystore::PasswordError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Terminal interaction failed: {0}")]
    Terminal(#[from] dialoguer::Error),

    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Errors that can occur during network operations
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Invalid network configuration: {0}")]
    InvalidConfig(String),

    #[error("Failed to connect to network: {0}")]
    ConnectionFailed(String),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Generic error: {0}")]
    Generic(String),
}

impl From<crate::error::CliError> for NetworkError {
    fn from(err: crate::error::CliError) -> Self {
        Self::AuthenticationFailed(err.to_string())
    }
}

impl From<KeystoreError> for NetworkError {
    fn from(err: KeystoreError) -> Self {
        Self::InvalidConfig(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for NetworkError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::Generic(err.to_string())
    }
}

impl From<dialoguer::Error> for NetworkError {
    fn from(err: dialoguer::Error) -> Self {
        Self::Generic(err.to_string())
    }
}

/// Errors that can occur during transaction operations
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transaction failed: {0}")]
    Failed(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),

    #[error("Integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<crate::error::CliError> for TransactionError {
    fn from(err: crate::error::CliError) -> Self {
        Self::Failed(err.to_string())
    }
}

/// Errors that can occur during balance operations
#[derive(Error, Debug)]
pub enum BalanceError {
    #[error("Failed to query balance: {0}")]
    QueryFailed(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Core provider error: {0}")]
    CoreProvider(String),

    #[error("Wallet error: {0}")]
    Wallet(#[from] rrelayer_core::WalletError),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),
}

impl From<crate::error::CliError> for BalanceError {
    fn from(err: crate::error::CliError) -> Self {
        Self::QueryFailed(err.to_string())
    }
}

/// Errors that can occur during signing operations
#[derive(Error, Debug)]
pub enum SigningError {
    #[error("Signing failed: {0}")]
    Failed(String),

    #[error("Wallet error: {0}")]
    Wallet(#[from] rrelayer_core::WalletError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Terminal interaction failed: {0}")]
    Terminal(#[from] dialoguer::Error),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<crate::error::CliError> for SigningError {
    fn from(err: crate::error::CliError) -> Self {
        Self::Failed(err.to_string())
    }
}

/// Errors that can occur during API key operations
#[derive(Error, Debug)]
pub enum ApiKeyError {
    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),
}

impl From<crate::error::CliError> for ApiKeyError {
    fn from(err: crate::error::CliError) -> Self {
        Self::SdkApi(rrelayer_sdk::ApiSdkError::ConfigError(err.to_string()))
    }
}

/// Errors that can occur during user management operations
#[derive(Error, Debug)]
pub enum UserError {
    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),
}

impl From<crate::error::CliError> for UserError {
    fn from(err: crate::error::CliError) -> Self {
        Self::SdkApi(rrelayer_sdk::ApiSdkError::ConfigError(err.to_string()))
    }
}

/// Errors that can occur during allowlist operations
#[derive(Error, Debug)]
pub enum AllowlistError {
    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),
}

impl From<crate::error::CliError> for AllowlistError {
    fn from(err: crate::error::CliError) -> Self {
        Self::SdkApi(rrelayer_sdk::ApiSdkError::ConfigError(err.to_string()))
    }
}

/// Errors that can occur during project initialization  
#[derive(Error, Debug)]
pub enum InitError {
    #[error("Invalid project configuration: {0}")]
    InvalidConfig(String),

    #[error("Wallet error: {0}")]
    Wallet(#[from] rrelayer_core::WalletError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration write error: {0}")]
    ConfigWrite(#[from] rrelayer_core::WriteFileError),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Terminal interaction failed: {0}")]
    Terminal(#[from] dialoguer::Error),
}

impl From<KeystoreError> for InitError {
    fn from(err: KeystoreError) -> Self {
        Self::InvalidConfig(err.to_string())
    }
}

/// Errors that can occur during configuration operations
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration write error: {0}")]
    ConfigWrite(#[from] rrelayer_core::WriteFileError),
}

impl From<crate::error::CliError> for ConfigError {
    fn from(err: crate::error::CliError) -> Self {
        Self::Invalid(err.to_string())
    }
}

impl From<rrelayer_sdk::ApiSdkError> for ConfigError {
    fn from(err: rrelayer_sdk::ApiSdkError) -> Self {
        Self::Invalid(err.to_string())
    }
}

/// Errors that can occur during project startup operations
#[derive(Error, Debug)]
pub enum ProjectStartupError {
    #[error("Project not initialized: {0}")]
    NotInitialized(String),

    #[error("Docker operation failed: {0}")]
    DockerFailed(String),

    #[error("Project configuration invalid: {0}")]
    InvalidConfig(String),

    #[error("Docker compose not found")]
    DockerComposeNotFound,

    #[error("Environment variable missing: {0}")]
    MissingEnvironmentVariable(String),

    #[error("Core startup error: {0}")]
    CoreStartup(#[from] rrelayer_core::StartError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PostgreSQL connection error: {0}")]
    PostgresConnection(#[from] rrelayer_core::PostgresConnectionError),
}

impl From<&str> for ProjectStartupError {
    fn from(err: &str) -> Self {
        ProjectStartupError::DockerFailed(err.to_string())
    }
}

/// Errors that can occur during relayer management operations (list, create, clone)
#[derive(Error, Debug)]
pub enum RelayerManagementError {
    #[error("Failed to create relayer: {0}")]
    CreationFailed(String),

    #[error("SDK API error: {0}")]
    SdkApi(#[from] rrelayer_sdk::ApiSdkError),

    #[error("Project configuration error: {0}")]
    ProjectConfig(String),
}

impl From<crate::error::CliError> for RelayerManagementError {
    fn from(err: crate::error::CliError) -> Self {
        Self::CreationFailed(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for RelayerManagementError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::CreationFailed(err.to_string())
    }
}

impl From<KeystoreError> for RelayerManagementError {
    fn from(err: KeystoreError) -> Self {
        Self::ProjectConfig(err.to_string())
    }
}
