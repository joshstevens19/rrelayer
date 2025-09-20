use std::sync::Arc;

use alloy::consensus::{SignableTransaction, TxEnvelope};
use alloy::network::{AnyNetwork, AnyTransactionReceipt};
use alloy::rpc::types::serde_helpers::WithOtherFields;
use alloy::{
    consensus::TypedTransaction,
    dyn_abi::eip712::TypedData,
    eips::{BlockId, BlockNumberOrTag},
    network::primitives::BlockTransactionsKind,
    network::Ethereum,
    network::TransactionBuilderError,
    primitives::PrimitiveSignature,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::{client::ClientBuilder, types::TransactionRequest},
    signers::local::LocalSignerError,
    transports::{
        http::{Client, Http},
        layers::{RetryBackoffLayer, RetryBackoffService},
        RpcError, TransportErrorKind,
    },
};
use alloy_eips::eip2718::Encodable2718;
use rand::{thread_rng, Rng};
use reqwest::Url;
use thiserror::Error;

use crate::wallet::{
    AwsKmsWalletManager, MnemonicWalletManager, PrivyWalletManager, WalletError, WalletManagerTrait,
};
use crate::yaml::AwsKmsSigningKey;
use crate::{
    gas::{
        blob_gas_oracle::{BlobGasEstimatorResult, BlobGasPriceResult},
        fee_estimator::base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult},
        types::GasLimit,
    },
    network::types::ChainId,
    rrelayer_info,
    shared::common_types::{EvmAddress, WalletOrProviderError},
    transaction::types::{TransactionHash, TransactionNonce},
    NetworkSetupConfig,
};

pub type RelayerProvider = RootProvider<RetryBackoffService<Http<Client>>, AnyNetwork>;

#[derive(Clone)]
pub struct EvmProvider {
    rpc_clients: Vec<Arc<RelayerProvider>>,
    wallet_manager: Arc<dyn WalletManagerTrait>,
    gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    pub chain_id: ChainId,
    pub name: String,
    pub provider_urls: Vec<String>,
    /// this is in milliseconds (min 250ms)
    pub blocks_every: u64,
    pub confirmations: u64,
}

/// Calculates the average block time difference by comparing recent blocks.
///
/// This function examines the timestamps of the last two blocks to determine
/// the average block time for the network. If there are insufficient blocks,
/// it defaults to 2 seconds.
///
/// # Arguments
/// * `provider` - The RPC provider to query blockchain data
///
/// # Returns
/// * `Ok(u64)` - Average block time in milliseconds (max 250ms)
/// * `Err(RpcError<TransportErrorKind>)` - RPC error if unable to fetch block data
pub async fn calculate_block_time_difference(
    provider: &RelayerProvider,
) -> Result<u64, RpcError<TransportErrorKind>> {
    let latest_block_number = provider.get_block_number().await?;

    // Ensure there's no underflow if not enough blocks to check set to 250ms (max limit)
    if latest_block_number <= 13 {
        rrelayer_info!("Not enough blocks to calculate block time difference, setting to 250ms");
        return Ok(250);
    }

    let latest = provider
        .get_block(
            BlockId::Number(BlockNumberOrTag::Number(latest_block_number - 12)),
            BlockTransactionsKind::Hashes,
        )
        .await?;
    let earliest = provider
        .get_block(
            BlockId::Number(BlockNumberOrTag::Number(latest_block_number - 13)),
            BlockTransactionsKind::Hashes,
        )
        .await?;

    let latest = latest.ok_or(RpcError::Transport(TransportErrorKind::Custom(
        "Latest block none".to_string().into(),
    )))?;
    let earliest = earliest.ok_or(RpcError::Transport(TransportErrorKind::Custom(
        "Earliest block none".to_string().into(),
    )))?;

    let block_time_seconds = latest.header.timestamp - earliest.header.timestamp;
    let block_time_ms = block_time_seconds * 1000;

    let limited_block_time_ms = std::cmp::max(block_time_ms, 250);

    rrelayer_info!(
        "Calculated block time: {}s ({}ms), limited to {}ms",
        block_time_seconds,
        block_time_ms,
        limited_block_time_ms
    );

    Ok(limited_block_time_ms)
}

#[derive(Error, Debug)]
pub enum RetryClientError {
    #[error("http provider cant be created for {0}: {1}")]
    HttpProviderCantBeCreated(String, String),
}

/// Creates a retry-enabled HTTP client for RPC communications.
///
/// This function sets up an HTTP client with automatic retry capabilities using
/// exponential backoff for handling transient network failures.
///
/// # Arguments
/// * `rpc_url` - The RPC endpoint URL to connect to
///
/// # Returns
/// * `Ok(Arc<RelayerProvider>)` - Configured provider with retry functionality
/// * `Err(RetryClientError)` - Error if the client cannot be created
pub fn create_retry_client(rpc_url: &str) -> Result<Arc<RelayerProvider>, RetryClientError> {
    let url = Url::parse(rpc_url).map_err(|e| {
        RetryClientError::HttpProviderCantBeCreated(rpc_url.to_string(), e.to_string())
    })?;

    // TODO: check this config
    let retry_layer = RetryBackoffLayer::new(5000, 500, 660);
    let client = ClientBuilder::default().layer(retry_layer).http(url.clone());

    let provider = ProviderBuilder::new().network::<AnyNetwork>().on_client(client);

    Ok(Arc::new(provider))
}

#[derive(Error, Debug)]
pub enum SendTransactionError {
    #[error("Wallet error: {0}")]
    WalletError(#[from] LocalSignerError),

    #[error("Transaction builder error: {0}")]
    TransactionBuilderError(#[from] TransactionBuilderError<Ethereum>),

    #[error("Provider error: {0}")]
    RpcError(#[from] RpcError<TransportErrorKind>),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Error, Debug)]
pub enum EvmProviderNewError {
    #[error("http provider cant be created for {0}: {1}")]
    HttpProviderCantBeCreated(String, String),

    #[error("wallet manager error: {0}")]
    WalletManagerError(String),

    #[error("{0}")]
    ProviderError(RpcError<TransportErrorKind>),
}

impl EvmProvider {
    /// Creates a new EvmProvider using a mnemonic phrase for wallet management.
    ///
    /// This constructor initializes an EvmProvider with a mnemonic-based wallet manager
    /// for signing transactions and managing addresses.
    ///
    /// # Arguments
    /// * `network_setup_config` - Network configuration including RPC URLs and chain settings
    /// * `mnemonic` - BIP39 mnemonic phrase for wallet derivation
    /// * `gas_estimator` - Gas fee estimation service for transaction pricing
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully initialized EvmProvider
    /// * `Err(EvmProviderNewError)` - Error during provider initialization
    pub async fn new_with_mnemonic(
        network_setup_config: &NetworkSetupConfig,
        mnemonic: &str,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(MnemonicWalletManager::new(mnemonic));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    /// Creates a new EvmProvider using Privy for wallet management.
    ///
    /// This constructor initializes an EvmProvider with Privy wallet management service
    /// for handling user authentication and wallet operations.
    ///
    /// # Arguments
    /// * `network_setup_config` - Network configuration including RPC URLs and chain settings
    /// * `app_id` - Privy application identifier
    /// * `app_secret` - Privy application secret for authentication
    /// * `gas_estimator` - Gas fee estimation service for transaction pricing
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully initialized EvmProvider
    /// * `Err(EvmProviderNewError)` - Error during provider initialization
    pub async fn new_with_privy(
        network_setup_config: &NetworkSetupConfig,
        app_id: String,
        app_secret: String,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let privy_manager = PrivyWalletManager::new(app_id, app_secret)
            .await
            .map_err(|e| EvmProviderNewError::WalletManagerError(e.to_string()))?;
        let wallet_manager = Arc::new(privy_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    /// Creates a new EvmProvider using AWS KMS for wallet management.
    ///
    /// This constructor initializes an EvmProvider with AWS KMS wallet management
    /// for signing transactions using AWS Key Management Service keys.
    ///
    /// # Arguments
    /// * `network_setup_config` - Network configuration including RPC URLs and chain settings
    /// * `aws_kms_config` - AWS KMS configuration containing key IDs, region, and credentials
    /// * `gas_estimator` - Gas fee estimation service for transaction pricing
    ///
    /// # Returns
    /// * `Ok(EvmProvider)` - Successfully initialized provider with AWS KMS wallet manager
    /// * `Err(EvmProviderNewError)` - Error if provider or AWS KMS setup fails
    pub async fn new_with_aws_kms(
        network_setup_config: &NetworkSetupConfig,
        aws_kms_config: AwsKmsSigningKey,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(AwsKmsWalletManager::new(aws_kms_config));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    /// Internal constructor for creating an EvmProvider with any wallet manager.
    ///
    /// This is the shared initialization logic used by both mnemonic and Privy constructors.
    /// It sets up RPC connections, determines chain ID, calculates block timing, and
    /// configures the provider with all necessary components.
    ///
    /// # Arguments
    /// * `network_setup_config` - Network configuration including RPC URLs and chain settings
    /// * `wallet_manager` - Wallet management implementation (mnemonic or Privy)
    /// * `gas_estimator` - Gas fee estimation service for transaction pricing
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully initialized EvmProvider
    /// * `Err(EvmProviderNewError)` - Error during provider initialization
    async fn new_internal(
        network_setup_config: &NetworkSetupConfig,
        wallet_manager: Arc<dyn WalletManagerTrait>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let provider =
            create_retry_client(&network_setup_config.provider_urls[0]).map_err(|e| {
                EvmProviderNewError::HttpProviderCantBeCreated(
                    network_setup_config.provider_urls[0].clone(),
                    e.to_string(),
                )
            })?;

        let chain_id = ChainId::new(
            provider.get_chain_id().await.map_err(EvmProviderNewError::ProviderError)?,
        );

        let mut providers: Vec<Arc<RelayerProvider>> = vec![provider.clone()];
        for url in network_setup_config.provider_urls.iter().skip(1) {
            providers.push(create_retry_client(url).map_err(|e| {
                EvmProviderNewError::HttpProviderCantBeCreated(url.clone(), e.to_string())
            })?);
        }

        Ok(EvmProvider {
            blocks_every: calculate_block_time_difference(&provider)
                .await
                .map_err(EvmProviderNewError::ProviderError)?,
            rpc_clients: providers,
            wallet_manager,
            gas_estimator,
            chain_id,
            name: network_setup_config.name.to_string(),
            provider_urls: network_setup_config.provider_urls.to_owned(),
            confirmations: network_setup_config.confirmations.unwrap_or(12),
        })
    }

    /// Returns a random RPC client from the configured providers for load balancing.
    ///
    /// This method randomly selects one of the available RPC providers to distribute
    /// load across multiple endpoints and improve reliability.
    ///
    /// # Returns
    /// * `Arc<RelayerProvider>` - A randomly selected RPC provider
    pub fn rpc_client(&self) -> Arc<RelayerProvider> {
        let mut rng = thread_rng();
        let index = rng.gen_range(0..self.rpc_clients.len());
        self.rpc_clients[index].clone()
    }

    /// Creates a new wallet at the specified index.
    ///
    /// This method generates a new wallet address using the configured wallet manager
    /// (either mnemonic-based or Privy-based) at the given derivation index.
    ///
    /// # Arguments
    /// * `wallet_index` - The derivation index for the new wallet
    ///
    /// # Returns
    /// * `Ok(EvmAddress)` - The generated wallet address
    /// * `Err(WalletError)` - Error if wallet creation fails
    pub async fn create_wallet(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.create_wallet(wallet_index, &self.chain_id).await
    }

    /// Retrieves the address for a wallet at the specified index.
    ///
    /// Gets the Ethereum address for a previously created or existing wallet
    /// at the given derivation index.
    ///
    /// # Arguments
    /// * `wallet_index` - The derivation index of the wallet
    ///
    /// # Returns
    /// * `Ok(EvmAddress)` - The wallet address
    /// * `Err(WalletError)` - Error if address retrieval fails
    pub async fn get_address(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.get_address(wallet_index, &self.chain_id).await
    }

    /// Retrieves the transaction receipt for a given transaction hash.
    ///
    /// Queries the blockchain for the receipt of a previously submitted transaction,
    /// which contains information about execution status, gas usage, and logs.
    ///
    /// # Arguments
    /// * `transaction_hash` - The hash of the transaction to query
    ///
    /// # Returns
    /// * `Ok(Some(AnyTransactionReceipt))` - Transaction receipt if found
    /// * `Ok(None)` - If transaction is not yet mined
    /// * `Err(RpcError<TransportErrorKind>)` - RPC error during query
    pub async fn get_receipt(
        &self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<AnyTransactionReceipt>, RpcError<TransportErrorKind>> {
        let receipt =
            self.rpc_client().get_transaction_receipt(transaction_hash.into_alloy_hash()).await?;

        Ok(receipt)
    }

    /// Retrieves the current transaction nonce for a wallet.
    ///
    /// Gets the next available nonce (transaction count) for the specified wallet,
    /// which is required for transaction ordering and replay protection.
    ///
    /// # Arguments
    /// * `wallet_index` - Index of the wallet to query
    ///
    /// # Returns
    /// * `Ok(TransactionNonce)` - The next available nonce
    /// * `Err(WalletOrProviderError)` - Error if nonce retrieval fails
    pub async fn get_nonce(
        &self,
        wallet_index: &u32,
    ) -> Result<TransactionNonce, WalletOrProviderError> {
        let address =
            self.wallet_manager.get_address(*wallet_index, &self.chain_id).await.map_err(|e| {
                WalletOrProviderError::InternalError(format!("Failed to get address: {}", e))
            })?;

        let nonce = self
            .rpc_client()
            .get_transaction_count(address.into_address())
            .block_id(BlockId::Number(BlockNumberOrTag::Pending))
            .await
            .map_err(WalletOrProviderError::ProviderError)?;

        Ok(TransactionNonce::new(nonce))
    }

    pub async fn get_nonce_from_address(
        &self,
        address: &EvmAddress,
    ) -> Result<TransactionNonce, RpcError<TransportErrorKind>> {
        let nonce = self
            .rpc_client()
            .get_transaction_count(address.into_address())
            .block_id(BlockId::Number(BlockNumberOrTag::Pending))
            .await?;

        Ok(TransactionNonce::new(nonce))
    }

    /// Signs and broadcasts a transaction to the network.
    ///
    /// This method signs the provided transaction with the specified wallet
    /// and submits it to the blockchain network for processing.
    ///
    /// # Arguments
    /// * `wallet_index` - Index of the wallet to use for signing
    /// * `transaction` - The transaction to sign and send
    ///
    /// # Returns
    /// * `Ok(TransactionHash)` - Hash of the submitted transaction
    /// * `Err(SendTransactionError)` - Error if signing or sending fails
    pub async fn send_transaction(
        &self,
        wallet_index: &u32,
        transaction: TypedTransaction,
    ) -> Result<TransactionHash, SendTransactionError> {
        let signature = self
            .wallet_manager
            .sign_transaction(*wallet_index, &transaction, &self.chain_id)
            .await
            .map_err(|e| SendTransactionError::InternalError(e.to_string()))?;

        let tx_envelope = match transaction {
            TypedTransaction::Legacy(tx) => TxEnvelope::Legacy(tx.into_signed(signature)),
            TypedTransaction::Eip2930(tx) => TxEnvelope::Eip2930(tx.into_signed(signature)),
            TypedTransaction::Eip1559(tx) => TxEnvelope::Eip1559(tx.into_signed(signature)),
            TypedTransaction::Eip4844(tx) => TxEnvelope::Eip4844(tx.into_signed(signature)),
            TypedTransaction::Eip7702(tx) => TxEnvelope::Eip7702(tx.into_signed(signature)),
        };

        let provider = self.rpc_client();
        let tx_bytes = tx_envelope.encoded_2718();
        let receipt = provider.send_raw_transaction(&tx_bytes).await?;

        Ok(TransactionHash::from_alloy_hash(receipt.tx_hash()))
    }

    /// Signs a transaction without broadcasting it.
    ///
    /// Creates a cryptographic signature for the transaction using the specified
    /// wallet, but does not submit it to the network.
    ///
    /// # Arguments
    /// * `wallet_index` - Index of the wallet to use for signing
    /// * `transaction` - The transaction to sign
    ///
    /// # Returns
    /// * `Ok(PrimitiveSignature)` - The transaction signature
    /// * `Err(WalletError)` - Error if signing fails
    pub async fn sign_transaction(
        &self,
        wallet_index: &u32,
        transaction: &TypedTransaction,
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_transaction(*wallet_index, transaction, &self.chain_id).await
    }

    /// Signs a text message using EIP-191 personal message signing.
    ///
    /// Creates a signature for arbitrary text data that can be used for
    /// authentication or message verification purposes.
    ///
    /// # Arguments
    /// * `wallet_index` - Index of the wallet to use for signing
    /// * `text` - The text message to sign
    ///
    /// # Returns
    /// * `Ok(PrimitiveSignature)` - The message signature
    /// * `Err(WalletError)` - Error if signing fails
    pub async fn sign_text(
        &self,
        wallet_index: &u32,
        text: &String,
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_text(*wallet_index, text).await
    }

    /// Signs structured data using EIP-712 typed data signing.
    ///
    /// Creates a signature for structured data following the EIP-712 standard,
    /// commonly used for smart contract interactions and off-chain signatures.
    ///
    /// # Arguments
    /// * `wallet_index` - Index of the wallet to use for signing
    /// * `typed_data` - The structured data to sign following EIP-712 format
    ///
    /// # Returns
    /// * `Ok(PrimitiveSignature)` - The typed data signature
    /// * `Err(WalletError)` - Error if signing fails
    pub async fn sign_typed_data(
        &self,
        wallet_index: &u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_typed_data(*wallet_index, typed_data).await
    }

    /// Estimates the gas required for a transaction execution.
    ///
    /// Simulates the transaction execution to determine the amount of gas
    /// that would be consumed, helping with accurate gas limit setting.
    ///
    /// # Arguments
    /// * `transaction` - The transaction to estimate gas for
    ///
    /// # Returns
    /// * `Ok(GasLimit)` - The estimated gas limit
    /// * `Err(RpcError<TransportErrorKind>)` - RPC error during estimation
    pub async fn estimate_gas(
        &self,
        transaction: &TypedTransaction,
        from: &EvmAddress,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        let mut request: TransactionRequest = transaction.clone().into();
        // need from here else it will fail gas estimating
        request.from = Some(from.into_address());

        let request_with_other = WithOtherFields::new(request);

        let result = self.rpc_client().estimate_gas(&request_with_other).await?;

        Ok(GasLimit::new(result as u128))
    }

    /// Calculates current gas prices for different transaction speeds.
    ///
    /// Uses the configured gas estimator to determine appropriate gas prices
    /// for slow, medium, fast, and super-fast transaction confirmation speeds.
    ///
    /// # Returns
    /// * `Ok(GasEstimatorResult)` - Gas price estimates for different speeds
    /// * `Err(GasEstimatorError)` - Error during gas price calculation
    pub async fn calculate_gas_price(&self) -> Result<GasEstimatorResult, GasEstimatorError> {
        self.gas_estimator.get_gas_prices(&self.chain_id).await
    }

    /// Retrieves the ETH balance for a given address.
    ///
    /// Queries the blockchain for the current ETH balance of the specified address.
    ///
    /// # Arguments
    /// * `address` - The Ethereum address to check the balance for
    ///
    /// # Returns
    /// * `Ok(alloy::primitives::U256)` - The balance in wei
    /// * `Err(RpcError<TransportErrorKind>)` - RPC error during balance query
    pub async fn get_balance(
        &self,
        address: &EvmAddress,
    ) -> Result<alloy::primitives::U256, RpcError<TransportErrorKind>> {
        let balance = self.rpc_client().get_balance(address.into_address()).await?;
        Ok(balance)
    }

    /// Checks if the current network supports blob transactions (EIP-4844).
    ///
    /// Blob transactions are a feature introduced in Ethereum's Dencun upgrade
    /// that allows for more efficient data availability for Layer 2 solutions.
    ///
    /// # Returns
    /// * `true` - If the network supports blob transactions
    /// * `false` - If blob transactions are not supported
    pub fn supports_blob_transactions(&self) -> bool {
        // Ethereum mainnet and testnet chain IDs that support blobs
        matches!(
            self.chain_id.u64(),
            1 |      // Ethereum Mainnet
           17000 |  // Holesky Testnet
           11155111 // Sepolia Testnet
        )
    }

    /// Calculates blob gas prices for Ethereum blob transactions (EIP-4844).
    ///
    /// This method determines the current blob gas prices for different transaction speeds
    /// on networks that support blob transactions. Blob transactions are used primarily
    /// by Layer 2 solutions for efficient data availability.
    ///
    /// # Returns
    /// * `Ok(BlobGasEstimatorResult)` - Blob gas price estimates for different speeds
    /// * `Err(anyhow::Error)` - Error if blob gas calculation fails or network doesn't support blobs
    pub async fn calculate_ethereum_blob_gas_price(
        &self,
    ) -> Result<BlobGasEstimatorResult, anyhow::Error> {
        let base_fee_per_blob_gas = match self.rpc_client().get_blob_base_fee().await {
            Ok(fee) => fee, // This is already a u128 value
            Err(_) => return Err(anyhow::anyhow!("Chain does not support blob transactions")),
        };

        // Base price calculations
        // Blob gas for a single blob is 128KB (131,072 gas units)
        let blob_gas_per_blob = 131_072; // 128 * 1024

        // Calculate fees with different multipliers for speeds
        let super_fast_multiplier = 1.5;
        let fast_multiplier = 1.2;
        let slow_multiplier = 0.8;

        let super_fast_price = (base_fee_per_blob_gas as f64 * super_fast_multiplier) as u128;
        let fast_price = (base_fee_per_blob_gas as f64 * fast_multiplier) as u128;
        let medium_price = base_fee_per_blob_gas;
        let slow_price = (base_fee_per_blob_gas as f64 * slow_multiplier) as u128;

        let super_fast_total = super_fast_price * blob_gas_per_blob;
        let fast_total = fast_price * blob_gas_per_blob;
        let medium_total = medium_price * blob_gas_per_blob;
        let slow_total = slow_price * blob_gas_per_blob;

        Ok(BlobGasEstimatorResult {
            super_fast: BlobGasPriceResult {
                blob_gas_price: super_fast_price,
                total_fee_for_blob: super_fast_total,
            },
            fast: BlobGasPriceResult { blob_gas_price: fast_price, total_fee_for_blob: fast_total },
            medium: BlobGasPriceResult {
                blob_gas_price: medium_price,
                total_fee_for_blob: medium_total,
            },
            slow: BlobGasPriceResult { blob_gas_price: slow_price, total_fee_for_blob: slow_total },
            base_fee_per_blob_gas,
            timestamp: chrono::Utc::now().timestamp() as u64,
        })
    }
}
