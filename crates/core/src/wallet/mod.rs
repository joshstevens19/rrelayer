mod mnemonic_wallet_manager;

use crate::common_types::EvmAddress;
use crate::network::types::ChainId;
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::primitives::PrimitiveSignature;
use async_trait::async_trait;
pub use mnemonic_wallet_manager::{generate_seed_phrase, MnemonicWalletManager};
pub use privy_wallet_manager::PrivyWalletManager;

mod mnemonic_signing_key_providers;
pub use mnemonic_signing_key_providers::{get_mnemonic_from_signing_key, keystore};
mod privy_wallet_manager;

#[derive(Debug)]
pub enum WalletSource {
    Mnemonic(String),
    Privy(PrivyWalletManager),
}

#[async_trait]
pub trait WalletManagerTrait: Send + Sync {
    /// Create a new wallet at the specified index for the given chain
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, Box<dyn std::error::Error + Send + Sync>>;

    /// Get the address of an existing wallet
    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, Box<dyn std::error::Error + Send + Sync>>;

    /// Sign a transaction with the specified wallet
    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, Box<dyn std::error::Error + Send + Sync>>;

    /// Sign text with the specified wallet
    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
    ) -> Result<PrimitiveSignature, Box<dyn std::error::Error + Send + Sync>>;

    /// Sign typed data with the specified wallet
    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, Box<dyn std::error::Error + Send + Sync>>;
}
