use crate::common_types::EvmAddress;
use crate::network::ChainId;
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::primitives::Signature;
use async_trait::async_trait;
use thiserror::Error;

mod mnemonic_wallet_manager;
pub use mnemonic_wallet_manager::{generate_seed_phrase, MnemonicWalletManager};

mod mnemonic_signing_key_providers;
pub use mnemonic_signing_key_providers::get_mnemonic_from_signing_key;

mod aws_kms_wallet_manager;
pub use aws_kms_wallet_manager::AwsKmsWalletManager;

mod privy_wallet_manager;
pub use privy_wallet_manager::PrivyWalletManager;

mod turnkey_wallet_manager;
pub use turnkey_wallet_manager::TurnkeyWalletManager;

mod private_key_wallet_manager;
pub use private_key_wallet_manager::PrivateKeyWalletManager;

mod pkcs11_wallet_manager;
pub use pkcs11_wallet_manager::Pkcs11WalletManager;

mod fireblocks_wallet_manager;
pub use fireblocks_wallet_manager::FireblocksWalletManager;

mod composite_wallet_manager;
pub use composite_wallet_manager::CompositeWalletManager;

use crate::shared::{internal_server_error, HttpError};

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

    #[error("Invalid wallet index {index}, max available: {max_index}")]
    InvalidIndex { index: u32, max_index: u32 },

    #[error("Private key error: {0}")]
    PrivateKeyError(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

impl From<WalletError> for HttpError {
    fn from(value: WalletError) -> Self {
        internal_server_error(Some(value.to_string()))
    }
}

impl From<alloy::signers::Error> for WalletError {
    fn from(error: alloy::signers::Error) -> Self {
        WalletError::GenericSignerError(format!("Alloy signer error: {}", error))
    }
}

impl From<alloy::dyn_abi::Error> for WalletError {
    fn from(error: alloy::dyn_abi::Error) -> Self {
        WalletError::Eip712Error(format!("EIP712 error: {}", error))
    }
}

pub struct WalletManagerCloneChain {
    pub cloned_from: ChainId,
    pub cloned_to: ChainId,
}

pub enum WalletManagerChainId {
    ChainId(ChainId),
    Cloned(WalletManagerCloneChain),
}

impl WalletManagerChainId {
    pub fn main(&self) -> &ChainId {
        match self {
            WalletManagerChainId::ChainId(chain_id) => chain_id,
            WalletManagerChainId::Cloned(chain) => &chain.cloned_to,
        }
    }

    pub fn cloned_from_chain_id_or_default(&self) -> &ChainId {
        match self {
            WalletManagerChainId::ChainId(chain_id) => chain_id,
            WalletManagerChainId::Cloned(chain) => &chain.cloned_from,
        }
    }
}

impl From<ChainId> for WalletManagerChainId {
    fn from(chain_id: ChainId) -> Self {
        WalletManagerChainId::ChainId(chain_id)
    }
}

#[async_trait]
pub trait WalletManagerTrait: Send + Sync {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: WalletManagerChainId,
    ) -> Result<EvmAddress, WalletError>;

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: WalletManagerChainId,
    ) -> Result<EvmAddress, WalletError>;

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError>;

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError>;

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError>;

    /// Returns whether this wallet manager supports EIP-4844 blob transactions
    fn supports_blobs(&self) -> bool;
}
