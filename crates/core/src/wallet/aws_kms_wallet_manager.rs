use crate::common_types::EvmAddress;
use crate::network::types::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::AwsKmsSigningKey;
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::network::TxSigner;
use alloy::primitives::PrimitiveSignature;
use alloy::signers::{aws::AwsSigner, Signer};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_kms::{config::Credentials, Client};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// AWS KMS-based wallet manager.
///
/// This manager instantiates and caches AWS KMS signers for different wallet indices and chain IDs.
/// Supports both single key configuration (all indices use same key) and multiple key
/// configuration (each index maps to a specific KMS key ID).
#[derive(Debug)]
pub struct AwsKmsWalletManager {
    config: AwsKmsSigningKey,
    signers: Arc<RwLock<HashMap<(u32, u64), AwsSigner>>>,
}

impl AwsKmsWalletManager {
    /// Creates a new AWS KMS wallet manager.
    ///
    /// This manager handles wallet operations by interfacing with AWS KMS keys.
    /// It maintains a cache of signer instances for performance optimization.
    ///
    /// # Arguments
    /// * `config` - AWS KMS configuration containing key IDs, region, and credentials
    ///
    /// # Returns
    /// * `AwsKmsWalletManager` - A new wallet manager instance
    pub fn new(config: AwsKmsSigningKey) -> Self {
        Self { config, signers: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Gets the KMS key ID for the specified wallet index.
    ///
    /// # Arguments
    /// * `wallet_index` - The wallet index to get the key for
    ///
    /// # Returns
    /// * `Ok(String)` - The KMS key ID to use
    /// * `Err(WalletError)` - If the wallet index is out of bounds
    fn get_key_id_for_index(&self, wallet_index: u32) -> Result<String, WalletError> {
        self.config
            .key_ids
            .get_key_for_index(wallet_index)
            .map(|key| key.to_string())
            .map_err(|e| WalletError::ConfigurationError { message: e })
    }

    /// Gets or initializes an AWS KMS signer for the specified wallet index and chain ID.
    ///
    /// # Arguments
    /// * `wallet_index` - The wallet index to use
    /// * `chain_id` - The blockchain network chain ID
    ///
    /// # Returns
    /// * `Ok(AwsSigner)` - A signer instance configured for the specified KMS key and chain
    /// * `Err(WalletError)` - If signer initialization fails
    async fn get_or_initialize_signer(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<AwsSigner, WalletError> {
        let chain_id_u64 = chain_id.u64();
        let cache_key = (wallet_index, chain_id_u64);

        // Check if signer already exists
        {
            let signers = self.signers.read().await;
            if let Some(signer) = signers.get(&cache_key) {
                return Ok(signer.clone());
            }
        }

        // Initialize new signer instance
        let key_id = self.get_key_id_for_index(wallet_index)?;
        let signer = self.initialize_aws_kms_signer(&key_id, Some(chain_id_u64)).await?;

        // Cache the signer
        {
            let mut signers = self.signers.write().await;
            signers.insert(cache_key, signer.clone());
        }

        Ok(signer)
    }

    /// Initializes an AWS KMS signer instance from the configuration.
    ///
    /// This configures the AWS KMS client and returns a signer that will use
    /// the specified KMS key for all cryptographic operations.
    async fn initialize_aws_kms_signer(
        &self,
        key_id: &str,
        chain_id: Option<u64>,
    ) -> Result<AwsSigner, WalletError> {
        // Build AWS config
        let mut aws_config_builder = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(self.config.region.clone()));

        // Add credentials if provided
        if let (Some(access_key), Some(secret_key)) =
            (&self.config.access_key_id, &self.config.secret_access_key)
        {
            let credentials = Credentials::new(
                access_key.clone(),
                secret_key.clone(),
                self.config.session_token.clone(),
                None,
                "rrelayer-aws-kms",
            );
            aws_config_builder = aws_config_builder.credentials_provider(credentials);
        }

        let shared_config = aws_config_builder.load().await;
        let client = Client::new(&shared_config);

        // Initialize AWS KMS signer instance
        let signer = AwsSigner::new(client, key_id.to_string(), chain_id).await.map_err(|e| {
            WalletError::ApiError { message: format!("Failed to initialize AWS KMS signer: {}", e) }
        })?;

        Ok(signer)
    }
}

#[async_trait]
impl WalletManagerTrait for AwsKmsWalletManager {
    /// Gets the wallet address using AWS KMS.
    ///
    /// The wallet_index determines which KMS key ID to use from the configured key_ids.
    /// For single key configuration, all indices use the same key.
    /// For multiple key configuration, the index maps to the array position.
    /// Returns the Ethereum address derived from the KMS key.
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        Ok(EvmAddress::from(alloy::signers::Signer::address(&signer)))
    }

    /// Gets the address of the AWS KMS wallet.
    ///
    /// The wallet_index determines which KMS key ID to use from the configured key_ids.
    /// Returns the Ethereum address derived from the KMS key.
    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        Ok(EvmAddress::from(alloy::signers::Signer::address(&signer)))
    }

    /// Signs a transaction using AWS KMS.
    ///
    /// The transaction hash is sent to AWS KMS for signing with the specified key.
    /// Private key material never leaves the AWS KMS hardware security module.
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

    /// Signs text using AWS KMS.
    ///
    /// The message hash is sent to AWS KMS for signing with the specified key.
    /// Private key material never leaves the AWS KMS hardware security module.
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

    /// Signs typed data using AWS KMS.
    ///
    /// The EIP-712 hash is sent to AWS KMS for signing with the specified key.
    /// Private key material never leaves the AWS KMS hardware security module.
    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        // For typed data signing, we use chain ID from the typed data or default to 1
        let chain_id_u64 = typed_data.domain().chain_id.map(|id| id.to::<u64>()).unwrap_or(1);
        let chain_id = ChainId::new(chain_id_u64);
        let signer = self.get_or_initialize_signer(wallet_index, &chain_id).await?;

        // Sign the EIP-712 hash using AWS KMS
        let hash = typed_data.eip712_signing_hash()?;
        let signature = signer.sign_hash(&hash).await?;
        Ok(signature)
    }
}
