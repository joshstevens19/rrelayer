use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::{AwsKmsSigningKey, KmsKeyConfig};
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::network::TxSigner;
use alloy::primitives::PrimitiveSignature;
use alloy::signers::{aws::AwsSigner, Signer};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_kms::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct AwsKmsWalletManager {
    config: AwsKmsSigningKey,
    signers: Arc<RwLock<HashMap<(u32, u64), AwsSigner>>>,
}

impl AwsKmsWalletManager {
    /// Creates a new AWS KMS wallet manager.
    pub fn new(config: AwsKmsSigningKey) -> Self {
        Self { config, signers: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Gets the KMS key config for the specified wallet index.
    fn get_key_config_for_index(&self, wallet_index: u32) -> Result<&KmsKeyConfig, WalletError> {
        let index = wallet_index as usize;
        if index >= self.config.key_configs.len() {
            return Err(WalletError::ConfigurationError {
                message: format!(
                    "Wallet index {} is out of bounds for {} KMS key configs",
                    wallet_index,
                    self.config.key_configs.len()
                ),
            });
        }
        Ok(&self.config.key_configs[index])
    }

    /// Gets or initializes an AWS KMS signer for the specified wallet index and chain ID.
    async fn get_or_initialize_signer(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<AwsSigner, WalletError> {
        let chain_id_u64 = chain_id.u64();
        let cache_key = (wallet_index, chain_id_u64);

        {
            let signers = self.signers.read().await;
            if let Some(signer) = signers.get(&cache_key) {
                return Ok(signer.clone());
            }
        }

        let key_config = self.get_key_config_for_index(wallet_index)?;
        let signer = self.initialize_aws_kms_signer(key_config, Some(chain_id_u64)).await?;

        {
            let mut signers = self.signers.write().await;
            signers.insert(cache_key, signer.clone());
        }

        Ok(signer)
    }

    /// Initializes an AWS KMS signer instance from the key configuration.
    async fn initialize_aws_kms_signer(
        &self,
        key_config: &KmsKeyConfig,
        chain_id: Option<u64>,
    ) -> Result<AwsSigner, WalletError> {
        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(key_config.region.clone()))
            .load()
            .await;

        let client = Client::new(&aws_config);

        let signer =
            AwsSigner::new(client, key_config.key_id.clone(), chain_id).await.map_err(|e| {
                WalletError::ApiError {
                    message: format!("Failed to initialize AWS KMS signer: {}", e),
                }
            })?;

        Ok(signer)
    }
}

#[async_trait]
impl WalletManagerTrait for AwsKmsWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        Ok(EvmAddress::from(alloy::signers::Signer::address(&signer)))
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        Ok(EvmAddress::from(alloy::signers::Signer::address(&signer)))
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;

        let signature = match transaction {
            TypedTransaction::Legacy(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip2930(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip1559(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip4844(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip7702(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
        };

        Ok(signature)
    }

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
    ) -> Result<PrimitiveSignature, WalletError> {
        // For text signing, we use chain ID 1 as default since it's not chain-specific
        let default_chain_id = ChainId::new(1);
        let signer = self.get_or_initialize_signer(wallet_index, &default_chain_id).await?;
        let signature = signer.sign_message(text.as_bytes()).await?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        // For typed data signing, we use chain ID from the typed data or default to 1
        let chain_id_u64 = typed_data.domain().chain_id.map(|id| id.to::<u64>()).unwrap_or(1);
        let chain_id = ChainId::new(chain_id_u64);
        let signer = self.get_or_initialize_signer(wallet_index, &chain_id).await?;

        let hash = typed_data.eip712_signing_hash()?;
        let signature = signer.sign_hash(&hash).await?;
        Ok(signature)
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
