use std::sync::Arc;

use alloy::dyn_abi::TypedData;
use alloy::{consensus::TypedTransaction, signers::Signature};
use async_trait::async_trait;

use crate::{
    network::ChainId,
    shared::common_types::EvmAddress,
    wallet::{WalletError, WalletManagerTrait},
};

/// A composite wallet manager that routes to different wallet managers based on wallet index
pub struct CompositeWalletManager {
    primary_manager: Arc<dyn WalletManagerTrait>,
    private_key_manager: Option<Arc<dyn WalletManagerTrait>>,
}

impl CompositeWalletManager {
    pub fn new(
        primary_manager: Arc<dyn WalletManagerTrait>,
        private_key_manager: Option<Arc<dyn WalletManagerTrait>>,
    ) -> Self {
        CompositeWalletManager { primary_manager, private_key_manager }
    }

    // TODO: not ideal route but only way i could find for now to work without a big refactor
    /// Determine if a wallet index is for a private key (high range)
    fn is_private_key_index(&self, wallet_index: u32) -> bool {
        wallet_index >= u32::MAX - 1000
    }

    /// Get the appropriate wallet manager for the given index
    fn get_manager_for_index(
        &self,
        wallet_index: u32,
    ) -> Result<&Arc<dyn WalletManagerTrait>, WalletError> {
        if self.is_private_key_index(wallet_index) {
            if let Some(ref manager) = self.private_key_manager {
                Ok(manager)
            } else {
                Err(WalletError::UnsupportedOperation(format!(
                    "Private key wallet index {} requested but no private key manager configured",
                    wallet_index
                )))
            }
        } else {
            Ok(&self.primary_manager)
        }
    }
}

#[async_trait]
impl WalletManagerTrait for CompositeWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let manager = self.get_manager_for_index(wallet_index)?;
        manager.create_wallet(wallet_index, chain_id).await
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let manager = self.get_manager_for_index(wallet_index)?;
        manager.get_address(wallet_index, chain_id).await
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let manager = self.get_manager_for_index(wallet_index)?;
        manager.sign_transaction(wallet_index, transaction, chain_id).await
    }

    async fn sign_text(&self, wallet_index: u32, text: &str) -> Result<Signature, WalletError> {
        let manager = self.get_manager_for_index(wallet_index)?;
        manager.sign_text(wallet_index, text).await
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        let manager = self.get_manager_for_index(wallet_index)?;
        manager.sign_typed_data(wallet_index, typed_data).await
    }

    fn supports_blobs(&self) -> bool {
        // Return true if either manager supports blobs
        let primary_supports = self.primary_manager.supports_blobs();
        let private_key_supports =
            self.private_key_manager.as_ref().is_some_and(|m| m.supports_blobs());
        primary_supports || private_key_supports
    }
}
