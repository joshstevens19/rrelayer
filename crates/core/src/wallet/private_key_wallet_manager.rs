use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::network::TxSigner;
use alloy::primitives::Signature;
use alloy::signers::{local::PrivateKeySigner, Signer};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct PrivateKeyWalletManager {
    wallets: Mutex<HashMap<u32, PrivateKeySigner>>,
    private_keys: Vec<String>,
}

impl PrivateKeyWalletManager {
    pub fn new(private_keys: Vec<String>) -> Self {
        PrivateKeyWalletManager { wallets: Mutex::new(HashMap::new()), private_keys }
    }

    /// Convert wallet index from WalletIndex system back to internal array index
    fn convert_to_internal_index(&self, wallet_index: u32) -> Result<usize, WalletError> {
        // Check if this is a private key index (high range: >= u32::MAX - 1000)
        if wallet_index >= u32::MAX - 1000 {
            // This is a private key index from the WalletIndex conversion, convert back to array index
            let internal_index = (u32::MAX - wallet_index) as usize;
            if internal_index < self.private_keys.len() {
                Ok(internal_index)
            } else {
                Err(WalletError::InvalidIndex {
                    index: wallet_index,
                    max_index: self.private_keys.len() as u32,
                })
            }
        } else {
            // This is a low-range index (0, 1, 2, etc.)
            // When PrivateKeyWalletManager is the only provider, treat as direct array access
            // When mixed with mnemonic wallets, these should fail gracefully

            // For now, treat all low-range indexes as direct array access
            // This allows the system to work when only private keys are configured
            if (wallet_index as usize) < self.private_keys.len() {
                Ok(wallet_index as usize)
            } else {
                Err(WalletError::InvalidIndex {
                    index: wallet_index,
                    max_index: self.private_keys.len() as u32,
                })
            }
        }
    }

    /// Retrieves or creates a wallet at the specified index for the given chain.
    async fn get_wallet(
        &self,
        index: u32,
        chain_id: &ChainId,
    ) -> Result<PrivateKeySigner, WalletError> {
        let mut wallets = self.wallets.lock().await;

        if let Some(wallet) = wallets.get(&index) {
            Ok(wallet.clone())
        } else {
            let internal_index = self.convert_to_internal_index(index)?;

            let private_key = self.private_keys.get(internal_index).ok_or_else(|| {
                WalletError::InvalidIndex { index, max_index: self.private_keys.len() as u32 }
            })?;

            let wallet = private_key
                .parse::<PrivateKeySigner>()
                .map_err(|e| WalletError::PrivateKeyError(format!("Invalid private key: {}", e)))?
                .with_chain_id(Some((*chain_id).into()));

            wallets.insert(index, wallet.clone());

            Ok(wallet)
        }
    }
}

#[async_trait]
impl WalletManagerTrait for PrivateKeyWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        // For private key wallets, we can only create wallets for available private keys
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
    ) -> Result<Signature, WalletError> {
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

    async fn sign_text(&self, wallet_index: u32, text: &str) -> Result<Signature, WalletError> {
        let wallet = self.get_wallet(wallet_index, &ChainId::default()).await?;
        let signature = wallet.sign_message(text.as_bytes()).await?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        let wallet = self.get_wallet(wallet_index, &ChainId::default()).await?;
        let signature = wallet.sign_dynamic_typed_data(typed_data).await?;
        Ok(signature)
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
