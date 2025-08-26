use crate::common_types::EvmAddress;
use crate::network::types::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::network::TxSigner;
use alloy::primitives::PrimitiveSignature;
use alloy::signers::{
    local::{
        coins_bip39::{English, Mnemonic},
        LocalSignerError, MnemonicBuilder, PrivateKeySigner,
    },
    Signer,
};
use async_trait::async_trait;
use rand::thread_rng;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct MnemonicWalletManager {
    wallets: Mutex<HashMap<u32, PrivateKeySigner>>,
    mnemonic: String,
}

impl MnemonicWalletManager {
    /// Creates a new MnemonicWalletManager with the provided mnemonic phrase.
    ///
    /// # Arguments
    /// * `mnemonic` - The mnemonic phrase to use for wallet derivation
    ///
    /// # Returns
    /// * A new MnemonicWalletManager instance
    pub fn new(mnemonic: &str) -> Self {
        MnemonicWalletManager {
            wallets: Mutex::new(HashMap::new()),
            mnemonic: mnemonic.to_string(),
        }
    }

    /// Retrieves or creates a wallet at the specified index for the given chain.
    ///
    /// This method uses mnemonic-based key derivation to generate a wallet at the specified
    /// index. Wallets are cached to avoid re-derivation on subsequent calls.
    ///
    /// # Arguments
    /// * `index` - The derivation index for the wallet (BIP-44 account index)
    /// * `chain_id` - The chain ID to configure the wallet for
    ///
    /// # Returns
    /// * `Ok(PrivateKeySigner)` - The wallet signer configured for the specified chain
    /// * `Err(LocalSignerError)` - If wallet derivation fails
    async fn get_wallet(
        &self,
        index: u32,
        chain_id: &ChainId,
    ) -> Result<PrivateKeySigner, LocalSignerError> {
        let mut wallets = self.wallets.lock().await;

        if let Some(wallet) = wallets.get(&index) {
            Ok(wallet.clone())
        } else {
            let wallet = MnemonicBuilder::<English>::default()
                .phrase::<&str>(&self.mnemonic)
                .index(index)?
                .build()?
                .with_chain_id(Some((*chain_id).into()));

            wallets.insert(index, wallet.clone());

            Ok(wallet)
        }
    }
}

#[async_trait]
impl WalletManagerTrait for MnemonicWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        // For mnemonic wallets, we can always derive any index
        // So creation is the same as getting the address
        let wallet = self.get_wallet(wallet_index, chain_id).await?;
        Ok(EvmAddress::new(wallet.address()))
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let wallet = self.get_wallet(wallet_index, chain_id).await?;
        Ok(EvmAddress::new(wallet.address()))
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallet = self.get_wallet(wallet_index, chain_id).await?;

        let signature = match transaction {
            TypedTransaction::Legacy(tx) => {
                let mut tx = tx.clone();
                wallet.sign_transaction(&mut tx).await?
            }
            TypedTransaction::Eip2930(tx) => {
                let mut tx = tx.clone();
                wallet.sign_transaction(&mut tx).await?
            }
            TypedTransaction::Eip1559(tx) => {
                let mut tx = tx.clone();
                wallet.sign_transaction(&mut tx).await?
            }
            TypedTransaction::Eip4844(tx) => {
                let mut tx = tx.clone();
                wallet.sign_transaction(&mut tx).await?
            }
            TypedTransaction::Eip7702(tx) => {
                let mut tx = tx.clone();
                wallet.sign_transaction(&mut tx).await?
            }
        };

        Ok(signature)
    }

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallet = self.get_wallet(wallet_index, &ChainId::default()).await?;
        let signature = wallet.sign_message(text.as_bytes()).await?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallet = self.get_wallet(wallet_index, &ChainId::default()).await?;
        let signature = wallet.sign_dynamic_typed_data(typed_data).await?;
        Ok(signature)
    }
}

/// Generates a new 24-word BIP39 mnemonic seed phrase.
///
/// This function creates a cryptographically secure random mnemonic phrase
/// using the BIP39 standard with 24 words from the English wordlist.
///
/// # Returns
/// * `Ok(String)` - A 24-word mnemonic phrase
/// * `Err(WalletError)` - If mnemonic generation fails
///
/// # Examples
/// ```
/// let mnemonic = generate_seed_phrase()?;
/// println!("Generated mnemonic: {}", mnemonic);
/// ```
pub fn generate_seed_phrase() -> Result<String, WalletError> {
    let mut rng = thread_rng();
    let mnemonic = Mnemonic::<English>::new_with_count(&mut rng, 24)
        .map_err(|e| WalletError::MnemonicError(format!("Failed to generate mnemonic: {}", e)))?;
    let phrase = mnemonic.to_phrase();
    Ok(phrase)
}
