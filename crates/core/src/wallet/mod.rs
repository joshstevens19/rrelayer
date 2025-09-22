mod mnemonic_wallet_manager;

use crate::common_types::EvmAddress;
use crate::network::ChainId;
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::primitives::PrimitiveSignature;
use async_trait::async_trait;
pub use mnemonic_wallet_manager::{generate_seed_phrase, MnemonicWalletManager};
pub use privy_wallet_manager::PrivyWalletManager;
use thiserror::Error;

mod mnemonic_signing_key_providers;
pub use mnemonic_signing_key_providers::get_mnemonic_from_signing_key;
mod aws_kms_wallet_manager;
mod privy_wallet_manager;
mod turnkey_wallet_manager;
pub use aws_kms_wallet_manager::AwsKmsWalletManager;
pub use turnkey_wallet_manager::TurnkeyWalletManager;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Signing key error: {0}")]
    SigningKeyError(#[from] alloy::signers::local::LocalSignerError),

    #[error("Generic signer error: {0}")]
    GenericSignerError(String),

    #[error("Network request failed: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Hex decoding error: {0}")]
    HexError(#[from] hex::FromHexError),

    #[error("String encoding error: {0}")]
    StringEncodingError(#[from] std::string::FromUtf8Error),

    #[error("RLP decoding error: {0}")]
    RlpError(#[from] alloy_rlp::Error),

    #[error("Signature parsing error: {0}")]
    SignatureError(#[from] alloy::primitives::SignatureError),

    #[error("Wallet not found at index {index}")]
    WalletNotFound { index: u32 },

    #[error("API error: {message}")]
    ApiError { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },

    #[error("Invalid wallet configuration: {message}")]
    ConfigurationError { message: String },

    #[error("No signing key configured")]
    NoSigningKey,

    #[error("Unsupported transaction type: {tx_type}")]
    UnsupportedTransactionType { tx_type: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Mnemonic generation error: {0}")]
    MnemonicError(String),

    #[error("Key derivation error: {0}")]
    KeyDerivationError(String),

    #[error("EIP712 signing error: {0}")]
    Eip712Error(String),
}

impl From<alloy::signers::Error> for WalletError {
    /// Converts an Alloy signer error into a WalletError.
    ///
    /// # Arguments
    /// * `error` - The Alloy signer error to convert
    ///
    /// # Returns
    /// * A WalletError with the GenericSignerError variant containing the formatted error message
    fn from(error: alloy::signers::Error) -> Self {
        WalletError::GenericSignerError(format!("Alloy signer error: {}", error))
    }
}

impl From<alloy::dyn_abi::Error> for WalletError {
    /// Converts an Alloy DynABI error into a WalletError.
    fn from(error: alloy::dyn_abi::Error) -> Self {
        WalletError::Eip712Error(format!("EIP712 error: {}", error))
    }
}

#[async_trait]
pub trait WalletManagerTrait: Send + Sync {
    /// Create a new wallet at the specified index for the given chain
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError>;

    /// Get the address of an existing wallet
    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError>;

    /// Sign a transaction with the specified wallet
    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, WalletError>;

    /// Sign text with the specified wallet
    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
    ) -> Result<PrimitiveSignature, WalletError>;

    /// Sign typed data with the specified wallet
    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError>;
}
