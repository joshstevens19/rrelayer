use crate::gas::BLOB_GAS_PER_BLOB;
use crate::provider::layer_extensions::RpcLoggingLayer;
use crate::relayer::Relayer;
use crate::wallet::{
    AwsKmsWalletManager, CompositeWalletManager, FireblocksWalletManager, ImportKeyResult,
    MnemonicWalletManager, Pkcs11WalletManager, PrivateKeyWalletManager, PrivyWalletManager,
    TurnkeyWalletManager, WalletError, WalletManagerChainId, WalletManagerCloneChain,
    WalletManagerTrait,
};
use crate::yaml::{
    AwsKmsSigningProviderConfig, FireblocksSigningProviderConfig, Pkcs11SigningProviderConfig,
    TurnkeySigningProviderConfig,
};
use crate::{
    gas::{
        BaseGasFeeEstimator, BlobGasEstimatorResult, BlobGasPriceResult, GasEstimatorError,
        GasEstimatorResult, GasLimit,
    },
    network::ChainId,
    shared::common_types::{EvmAddress, WalletOrProviderError},
    transaction::types::{TransactionHash, TransactionNonce},
    NetworkSetupConfig,
};
use alloy::consensus::{SignableTransaction, TxEnvelope};
use alloy::network::{AnyNetwork, AnyTransactionReceipt};
use alloy::rpc::client::RpcClient;
use alloy::rpc::types::serde_helpers::WithOtherFields;
use alloy::{
    consensus::TypedTransaction,
    dyn_abi::eip712::TypedData,
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    network::TransactionBuilderError,
    primitives::{Bytes, Signature},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::LocalSignerError,
    transports::{
        http::{reqwest::Error as ReqwestError, Client, Http},
        layers::RetryBackoffLayer,
        RpcError, TransportErrorKind,
    },
};
use alloy_eips::eip2718::Encodable2718;
use rand::{thread_rng, Rng};
use reqwest::Url;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::info;

pub type RelayerProvider = Box<dyn Provider<AnyNetwork> + Send + Sync>;

const BLOCK_GAS_LIMIT_CACHE_TTL: Duration = Duration::from_secs(600);

/// The exact EIP-2718 payload prepared for broadcast and its deterministic hash.
///
/// Keeping the bytes and hash together prevents recovery evidence from drifting
/// from the payload handed to the RPC provider.
#[derive(Clone, Debug)]
pub struct SignedTransaction {
    bytes: Bytes,
    hash: TransactionHash,
}

impl SignedTransaction {
    pub fn hash(&self) -> TransactionHash {
        self.hash
    }

    #[cfg(test)]
    pub(crate) fn for_test(hash: TransactionHash) -> Self {
        Self { bytes: Bytes::new(), hash }
    }
}

#[derive(Clone)]
struct BlockGasLimitCache {
    gas_limit: GasLimit,
    fetched_at: Instant,
}

#[derive(Clone)]
pub struct EvmProvider {
    rpc_clients: Vec<Arc<RelayerProvider>>,
    wallet_manager: Arc<dyn WalletManagerTrait>,
    gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    block_gas_limit_cache: Arc<Mutex<Option<BlockGasLimitCache>>>,
    pub chain_id: ChainId,
    pub name: String,
    pub provider_urls: Vec<String>,
    /// this is in milliseconds (min 250ms)
    pub blocks_every: u64,
    pub confirmations: u64,
    /// Whether this provider type supports cloning (for clone prevention logic)
    can_clone: bool,
}

async fn calculate_block_time_difference(
    provider: &RelayerProvider,
) -> Result<u64, RpcError<TransportErrorKind>> {
    let latest_block_number = provider.get_block_number().await?;

    // Ensure there's no underflow if not enough blocks to check set to 250ms (max limit)
    if latest_block_number <= 13 {
        info!("Not enough blocks to calculate block time difference, setting to 250ms");
        return Ok(250);
    }

    let latest = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Number(latest_block_number - 12)))
        .await?;
    let earliest = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Number(latest_block_number - 13)))
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

    info!(
        "Calculated block time: {}s ({}ms), limited to {}ms",
        block_time_seconds, block_time_ms, limited_block_time_ms
    );

    Ok(limited_block_time_ms)
}

#[derive(Error, Debug)]
pub enum RetryClientError {
    #[error("http provider can't be created for {0}: {1}")]
    HttpProviderCantBeCreated(String, String),

    #[error("Could not build client: {0}")]
    CouldNotBuildClient(#[from] ReqwestError),
}

pub async fn create_retry_client(rpc_url: &str) -> Result<Arc<RelayerProvider>, RetryClientError> {
    let rpc_url = Url::parse(rpc_url).map_err(|e| {
        RetryClientError::HttpProviderCantBeCreated(rpc_url.to_string(), e.to_string())
    })?;

    let client_with_auth = Client::builder().timeout(Duration::from_secs(15)).build()?;

    let logging_layer = RpcLoggingLayer::new(rpc_url.to_string());
    let http = Http::with_client(client_with_auth, rpc_url);
    let retry_layer = RetryBackoffLayer::new(5000, 1000, 660);
    let rpc_client =
        RpcClient::builder().layer(retry_layer).layer(logging_layer).transport(http, false);
    let provider =
        ProviderBuilder::new().network::<AnyNetwork>().connect_client(rpc_client.clone());

    Ok(Arc::new(Box::new(provider)))
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
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
    WalletManagerError(#[from] WalletError),

    #[error("{0}")]
    ProviderError(RpcError<TransportErrorKind>),
}

impl EvmProvider {
    pub async fn new_with_mnemonic(
        network_setup_config: &NetworkSetupConfig,
        mnemonic: &str,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(MnemonicWalletManager::new(mnemonic));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    pub async fn new_with_privy(
        network_setup_config: &NetworkSetupConfig,
        app_id: String,
        app_secret: String,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let privy_manager = PrivyWalletManager::new(app_id, app_secret).await?;
        let wallet_manager = Arc::new(privy_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    pub async fn new_with_aws_kms(
        network_setup_config: &NetworkSetupConfig,
        aws_kms_config: AwsKmsSigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(AwsKmsWalletManager::new(aws_kms_config));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    pub async fn new_with_turnkey(
        network_setup_config: &NetworkSetupConfig,
        turnkey_config: TurnkeySigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let turnkey_manager = TurnkeyWalletManager::new(turnkey_config).await?;
        let wallet_manager = Arc::new(turnkey_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    pub async fn new_with_private_keys(
        network_setup_config: &NetworkSetupConfig,
        private_keys: Vec<String>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(PrivateKeyWalletManager::new(private_keys));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, false).await
    }

    pub async fn new_with_pkcs11(
        network_setup_config: &NetworkSetupConfig,
        pkcs11_config: Pkcs11SigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(Pkcs11WalletManager::new(pkcs11_config)?);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    pub async fn new_with_fireblocks(
        network_setup_config: &NetworkSetupConfig,
        fireblocks_config: FireblocksSigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let fireblocks_manager = FireblocksWalletManager::new(fireblocks_config).await?;
        let wallet_manager = Arc::new(fireblocks_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, false).await
    }

    pub async fn new_with_composite(
        network_setup_config: &NetworkSetupConfig,
        primary_manager: Arc<dyn WalletManagerTrait>,
        private_keys: Option<Vec<String>>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let private_key_manager = private_keys.map(|private_keys| {
            Arc::new(PrivateKeyWalletManager::new(private_keys)) as Arc<dyn WalletManagerTrait>
        });

        let wallet_manager =
            Arc::new(CompositeWalletManager::new(primary_manager, private_key_manager));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, true).await
    }

    async fn new_internal(
        network_setup_config: &NetworkSetupConfig,
        wallet_manager: Arc<dyn WalletManagerTrait>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        can_clone: bool,
    ) -> Result<Self, EvmProviderNewError> {
        let provider =
            create_retry_client(&network_setup_config.provider_urls[0]).await.map_err(|e| {
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
            providers.push(create_retry_client(url).await.map_err(|e| {
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
            block_gas_limit_cache: Arc::new(Mutex::new(None)),
            chain_id,
            name: network_setup_config.name.to_string(),
            provider_urls: network_setup_config.provider_urls.to_owned(),
            confirmations: network_setup_config.confirmations.unwrap_or(12),
            can_clone,
        })
    }

    pub fn rpc_client(&self) -> Arc<RelayerProvider> {
        let mut rng = thread_rng();
        let index = rng.gen_range(0..self.rpc_clients.len());
        self.rpc_clients[index].clone()
    }

    pub async fn clone_wallet(&self, relayer: &Relayer) -> Result<EvmAddress, WalletError> {
        let chain_id = WalletManagerChainId::Cloned(WalletManagerCloneChain {
            cloned_from: relayer.chain_id,
            cloned_to: self.chain_id,
        });

        self.wallet_manager.create_wallet(relayer.wallet_index(), chain_id).await
    }

    pub async fn create_wallet(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.create_wallet(wallet_index, self.chain_id.into()).await
    }

    pub async fn get_address(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.get_address(wallet_index, self.chain_id.into()).await
    }

    pub fn can_clone(&self) -> bool {
        self.can_clone
    }

    /// Returns whether this provider supports importing existing keys
    pub fn supports_key_import(&self) -> bool {
        self.wallet_manager.supports_key_import()
    }

    /// Imports an existing key into the wallet manager.
    /// Only supported for certain wallet managers (e.g., AWS KMS).
    /// Verifies the key's address matches expected_address before making any changes.
    pub async fn import_existing_key(
        &self,
        key_id: &str,
        wallet_index: u32,
        expected_address: &EvmAddress,
    ) -> Result<ImportKeyResult, WalletError> {
        self.wallet_manager
            .import_existing_key(key_id, wallet_index, &self.chain_id, expected_address)
            .await
    }

    pub async fn get_receipt(
        &self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<AnyTransactionReceipt>, RpcError<TransportErrorKind>> {
        let receipt =
            self.rpc_client().get_transaction_receipt(transaction_hash.into_alloy_hash()).await?;

        Ok(receipt)
    }

    /// Returns true when any configured RPC knows the transaction, false only
    /// when every configured RPC conclusively reports it absent, and an error
    /// when absence cannot be proven.
    pub async fn transaction_exists(
        &self,
        transaction_hash: &TransactionHash,
    ) -> Result<bool, RpcError<TransportErrorKind>> {
        transaction_exists_across_clients(&self.rpc_clients, transaction_hash).await
    }

    pub async fn get_nonce(
        &self,
        relayer: &Relayer,
    ) -> Result<TransactionNonce, WalletOrProviderError> {
        let address = self
            .wallet_manager
            .get_address(relayer.wallet_index(), relayer.wallet_manager_chain_id())
            .await
            .map_err(|e| {
                WalletOrProviderError::InternalError(format!("Failed to get address: {}", e))
            })?;

        let nonce = pending_nonce_across_clients(&self.rpc_clients, &address)
            .await
            .map_err(WalletOrProviderError::ProviderError)?;

        Ok(TransactionNonce::new(nonce))
    }

    pub async fn get_nonce_from_address(
        &self,
        address: &EvmAddress,
    ) -> Result<TransactionNonce, RpcError<TransportErrorKind>> {
        let nonce = pending_nonce_across_clients(&self.rpc_clients, address).await?;

        Ok(TransactionNonce::new(nonce))
    }

    pub async fn send_transaction(
        &self,
        relayer: &Relayer,
        transaction: TypedTransaction,
    ) -> Result<TransactionHash, SendTransactionError> {
        let signed = self.prepare_signed_transaction(relayer, &transaction).await?;
        self.send_raw_transaction(&signed).await
    }

    pub async fn prepare_signed_transaction(
        &self,
        relayer: &Relayer,
        transaction: &TypedTransaction,
    ) -> Result<SignedTransaction, SendTransactionError> {
        let signature = self
            .sign_transaction(relayer, transaction)
            .await
            .map_err(|e| SendTransactionError::InternalError(e.to_string()))?;

        Ok(Self::signed_transaction(transaction.clone(), signature))
    }

    pub async fn send_signed_transaction(
        &self,
        transaction: TypedTransaction,
        signature: Signature,
    ) -> Result<TransactionHash, SendTransactionError> {
        let signed = Self::signed_transaction(transaction, signature);
        self.send_raw_transaction(&signed).await
    }

    fn signed_transaction(
        transaction: TypedTransaction,
        signature: Signature,
    ) -> SignedTransaction {
        let tx_envelope = match transaction {
            TypedTransaction::Legacy(tx) => TxEnvelope::Legacy(tx.into_signed(signature)),
            TypedTransaction::Eip2930(tx) => TxEnvelope::Eip2930(tx.into_signed(signature)),
            TypedTransaction::Eip1559(tx) => TxEnvelope::Eip1559(tx.into_signed(signature)),
            TypedTransaction::Eip4844(tx) => TxEnvelope::Eip4844(tx.into_signed(signature)),
            TypedTransaction::Eip7702(tx) => TxEnvelope::Eip7702(tx.into_signed(signature)),
        };

        // For EIP-4844, the bytes sent over the wire include the blob sidecar,
        // but the transaction identity is the hash of the signed inner
        // transaction. Alloy caches that consensus hash on the envelope.
        let hash = TransactionHash::from_alloy_hash(tx_envelope.hash());
        let bytes = Bytes::from(tx_envelope.encoded_2718());

        SignedTransaction { bytes, hash }
    }

    pub async fn send_raw_transaction(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<TransactionHash, SendTransactionError> {
        let _ = self.rpc_client().send_raw_transaction(&transaction.bytes).await?;

        Ok(transaction.hash)
    }

    pub async fn sign_transaction(
        &self,
        relayer: &Relayer,
        transaction: &TypedTransaction,
    ) -> Result<Signature, WalletError> {
        self.wallet_manager
            .sign_transaction(
                relayer.wallet_index(),
                transaction,
                relayer.wallet_manager_chain_id(),
            )
            .await
    }

    pub async fn sign_text(&self, relayer: &Relayer, text: &str) -> Result<Signature, WalletError> {
        self.wallet_manager
            .sign_text(relayer.wallet_index(), text, relayer.wallet_manager_chain_id())
            .await
    }

    pub async fn sign_typed_data(
        &self,
        relayer: &Relayer,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        self.wallet_manager
            .sign_typed_data(relayer.wallet_index(), typed_data, relayer.wallet_manager_chain_id())
            .await
    }

    pub async fn estimate_gas(
        &self,
        transaction: &TypedTransaction,
        from: &EvmAddress,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        let mut request: TransactionRequest = transaction.clone().into();
        // need from here else it will fail gas estimating
        request.from = Some(from.into_address());

        let request_with_other = WithOtherFields::new(request);

        let result = self.rpc_client().estimate_gas(request_with_other).await?;

        Ok(GasLimit::new(result as u128))
    }

    /// Returns the latest block gas limit, cached briefly to avoid repeated RPC calls.
    ///
    /// A single transaction cannot fit in a block if its gas limit exceeds this value, but this is
    /// not a permanent chain constant. Validators/sequencers can adjust the block gas limit over
    /// time, so the cache is intentionally short-lived.
    pub async fn get_block_gas_limit(&self) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        {
            let cache = self.block_gas_limit_cache.lock().await;
            if let Some(cached) = cache.as_ref() {
                if cached.fetched_at.elapsed() < BLOCK_GAS_LIMIT_CACHE_TTL {
                    return Ok(cached.gas_limit);
                }
            }
        }

        let block_result = self.rpc_client().get_block_by_number(BlockNumberOrTag::Latest).await;
        let block = match block_result {
            Ok(Some(block)) => block,
            Ok(None) | Err(_) => {
                // Serve the expired cached value on a transient RPC failure - the block
                // gas limit moves slowly, and failing here would bubble up into the send
                // path for a transaction that already holds a reserved nonce.
                let cache = self.block_gas_limit_cache.lock().await;
                if let Some(cached) = cache.as_ref() {
                    return Ok(cached.gas_limit);
                }
                return match block_result {
                    Err(e) => Err(e),
                    _ => Err(RpcError::Transport(TransportErrorKind::Custom(
                        "Latest block not found".to_string().into(),
                    ))),
                };
            }
        };

        let gas_limit = GasLimit::new(block.header.gas_limit as u128);
        let mut cache = self.block_gas_limit_cache.lock().await;
        *cache = Some(BlockGasLimitCache { gas_limit, fetched_at: Instant::now() });

        Ok(gas_limit)
    }

    pub async fn calculate_gas_price(&self) -> Result<GasEstimatorResult, GasEstimatorError> {
        self.gas_estimator.get_gas_prices(&self.chain_id).await
    }

    pub async fn get_balance(
        &self,
        address: &EvmAddress,
    ) -> Result<alloy::primitives::U256, RpcError<TransportErrorKind>> {
        let balance = self.rpc_client().get_balance(address.into_address()).await?;
        Ok(balance)
    }

    /// Checks if the current network supports blob transactions (EIP-4844).
    pub fn supports_blob_transactions(&self) -> bool {
        matches!(
            self.chain_id.u64(),
            1 |       // Ethereum Mainnet
           17000 |    // Holesky Testnet
           11155111 | // Sepolia Testnet
            31337 // anvil fork
        )
    }

    /// Calculates blob gas prices for Ethereum blob transactions (EIP-4844).
    pub async fn calculate_ethereum_blob_gas_price(
        &self,
    ) -> Result<BlobGasEstimatorResult, anyhow::Error> {
        let base_fee_per_blob_gas = match self.rpc_client().get_blob_base_fee().await {
            Ok(fee) => fee,
            Err(_) => return Err(anyhow::anyhow!("Chain does not support blob transactions")),
        };

        let super_fast_price = (base_fee_per_blob_gas as f64 * 1.5) as u128;
        let fast_price = (base_fee_per_blob_gas as f64 * 1.2) as u128;
        let medium_price = base_fee_per_blob_gas;
        let slow_price = (base_fee_per_blob_gas as f64 * 0.8) as u128;

        let super_fast_total = super_fast_price * BLOB_GAS_PER_BLOB;
        let fast_total = fast_price * BLOB_GAS_PER_BLOB;
        let medium_total = medium_price * BLOB_GAS_PER_BLOB;
        let slow_total = slow_price * BLOB_GAS_PER_BLOB;

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

    pub fn supports_blobs(&self) -> bool {
        self.wallet_manager.supports_blobs()
    }
}

async fn pending_nonce_across_clients(
    rpc_clients: &[Arc<RelayerProvider>],
    address: &EvmAddress,
) -> Result<u64, RpcError<TransportErrorKind>> {
    if rpc_clients.is_empty() {
        return Err(RpcError::Transport(TransportErrorKind::Custom(
            "no RPC providers configured".to_string().into(),
        )));
    }

    let mut max_nonce: Option<u64> = None;
    let mut first_error = None;

    for rpc_client in rpc_clients {
        match rpc_client
            .get_transaction_count(address.into_address())
            .block_id(BlockId::Number(BlockNumberOrTag::Pending))
            .await
        {
            Ok(nonce) => max_nonce = Some(max_nonce.map_or(nonce, |max| max.max(nonce))),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    max_nonce.ok_or_else(|| {
        RpcError::Transport(TransportErrorKind::Custom(
            "no RPC providers configured".to_string().into(),
        ))
    })
}

async fn transaction_exists_across_clients(
    rpc_clients: &[Arc<RelayerProvider>],
    transaction_hash: &TransactionHash,
) -> Result<bool, RpcError<TransportErrorKind>> {
    if rpc_clients.is_empty() {
        return Err(RpcError::Transport(TransportErrorKind::Custom(
            "no RPC providers configured".to_string().into(),
        )));
    }

    let mut first_error = None;

    for rpc_client in rpc_clients {
        match rpc_client.get_transaction_by_hash(transaction_hash.into_alloy_hash()).await {
            Ok(Some(_)) => return Ok(true),
            Ok(None) => {}
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relayer::RelayerId;
    use crate::wallet::WalletManagerChainId;
    use alloy::{
        consensus::{TxEip4844, TxEip4844Variant, TxEip4844WithSidecar},
        primitives::{keccak256, Address, TxHash, U256},
        providers::ProviderBuilder,
        transports::mock::Asserter,
    };
    use alloy_eips::{eip4844::BlobTransactionSidecar, eip7594::BlobTransactionSidecarVariant};
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::json;
    use tokio::sync::Mutex;

    fn mock_client(asserter: Asserter) -> Arc<RelayerProvider> {
        let provider =
            ProviderBuilder::new().network::<AnyNetwork>().connect_mocked_client(asserter);
        Arc::new(Box::new(provider))
    }

    fn rpc_transaction(hash: TxHash) -> serde_json::Value {
        json!({
            "blockHash": null,
            "blockNumber": null,
            "hash": hash.to_string(),
            "transactionIndex": null,
            "type": "0x0",
            "nonce": "0x0",
            "input": "0x",
            "r": "0x3b08715b4403c792b8c7567edea634088bedcd7f60d9352b1f16c69830f3afd5",
            "s": "0x10b9afb67d2ec8b956f0e1dbc07eb79152904f3a7bf789fc869db56320adfe09",
            "chainId": "0x1",
            "v": "0x1c",
            "gas": "0x5208",
            "from": "0x32be343b94f860124dc4fee278fdcbd38c102d88",
            "to": "0xdf190dc7190dfba737d7777a163445b7fff16133",
            "value": "0x0",
            "gasPrice": "0x1"
        })
    }

    #[test]
    fn blob_transaction_hash_excludes_the_network_sidecar() {
        let transaction = TypedTransaction::Eip4844(TxEip4844Variant::TxEip4844WithSidecar(
            TxEip4844WithSidecar {
                tx: TxEip4844 {
                    chain_id: 1,
                    nonce: 1,
                    max_priority_fee_per_gas: 1,
                    max_fee_per_gas: 2,
                    gas_limit: 100_000,
                    to: Address::ZERO,
                    value: U256::ZERO,
                    access_list: Default::default(),
                    blob_versioned_hashes: vec![TxHash::repeat_byte(1)],
                    max_fee_per_blob_gas: 1,
                    input: Bytes::new(),
                },
                sidecar: BlobTransactionSidecarVariant::Eip4844(BlobTransactionSidecar {
                    blobs: vec![[2; 131_072].into()],
                    commitments: vec![[3; 48].into()],
                    proofs: vec![[4; 48].into()],
                }),
            },
        ));
        let signature = Signature::test_signature().with_parity(true);
        let expected_envelope = TxEnvelope::Eip4844(match transaction.clone() {
            TypedTransaction::Eip4844(tx) => tx.into_signed(signature),
            _ => unreachable!(),
        });
        let network_payload_hash = keccak256(expected_envelope.encoded_2718());

        let signed = EvmProvider::signed_transaction(transaction, signature);

        assert_eq!(signed.hash(), TransactionHash::from_alloy_hash(expected_envelope.hash()));
        assert_ne!(signed.hash(), TransactionHash::from_alloy_hash(&network_payload_hash));
    }

    #[tokio::test]
    async fn transaction_absence_requires_every_configured_provider_to_miss() {
        let hash = TxHash::repeat_byte(1);
        let first = Asserter::new();
        first.push_success(&serde_json::Value::Null);
        let second = Asserter::new();
        second.push_success(&serde_json::Value::Null);
        let clients = vec![mock_client(first), mock_client(second)];

        let exists = transaction_exists_across_clients(&clients, &TransactionHash::new(hash)).await;

        assert!(!exists.unwrap());
    }

    #[tokio::test]
    async fn transaction_exists_when_any_configured_provider_finds_hash() {
        let hash = TxHash::repeat_byte(2);
        let first = Asserter::new();
        first.push_success(&serde_json::Value::Null);
        let second = Asserter::new();
        second.push_success(&rpc_transaction(hash));
        let clients = vec![mock_client(first), mock_client(second)];

        let exists = transaction_exists_across_clients(&clients, &TransactionHash::new(hash)).await;

        assert!(exists.unwrap());
    }

    #[tokio::test]
    async fn transaction_absence_fails_closed_when_any_provider_errors() {
        let hash = TxHash::repeat_byte(3);
        let first = Asserter::new();
        first.push_success(&serde_json::Value::Null);
        let second = Asserter::new();
        second.push_failure_msg("backend unavailable");
        let clients = vec![mock_client(first), mock_client(second)];

        let exists = transaction_exists_across_clients(&clients, &TransactionHash::new(hash)).await;

        assert!(exists.is_err());
    }

    #[tokio::test]
    async fn pending_nonce_uses_maximum_only_when_every_provider_responds() {
        let first = Asserter::new();
        first.push_success(&"0x7");
        let second = Asserter::new();
        second.push_success(&"0x35");
        let clients = vec![mock_client(first), mock_client(second)];

        let nonce = pending_nonce_across_clients(&clients, &EvmAddress::zero()).await;

        assert_eq!(nonce.unwrap(), 53);
    }

    #[tokio::test]
    async fn pending_nonce_fails_closed_when_any_provider_errors() {
        let first = Asserter::new();
        first.push_success(&"0x35");
        let second = Asserter::new();
        second.push_failure_msg("backend unavailable");
        let clients = vec![mock_client(first), mock_client(second)];

        let nonce = pending_nonce_across_clients(&clients, &EvmAddress::zero()).await;

        assert!(nonce.is_err());
    }

    struct RecordingWalletManager {
        last_create_chain: Arc<Mutex<Option<(u32, u64, u64)>>>,
        address: EvmAddress,
    }

    #[async_trait]
    impl WalletManagerTrait for RecordingWalletManager {
        async fn create_wallet(
            &self,
            wallet_index: u32,
            chain_id: WalletManagerChainId,
        ) -> Result<EvmAddress, WalletError> {
            match chain_id {
                WalletManagerChainId::Cloned(chain) => {
                    let mut last_create_chain = self.last_create_chain.lock().await;
                    *last_create_chain =
                        Some((wallet_index, chain.cloned_from.u64(), chain.cloned_to.u64()));
                }
                WalletManagerChainId::ChainId(chain_id) => {
                    let mut last_create_chain = self.last_create_chain.lock().await;
                    *last_create_chain = Some((wallet_index, chain_id.u64(), chain_id.u64()));
                }
            }

            Ok(self.address)
        }

        async fn get_address(
            &self,
            _wallet_index: u32,
            _chain_id: WalletManagerChainId,
        ) -> Result<EvmAddress, WalletError> {
            Ok(self.address)
        }

        async fn sign_transaction(
            &self,
            _wallet_index: u32,
            _transaction: &TypedTransaction,
            _chain_id: WalletManagerChainId,
        ) -> Result<Signature, WalletError> {
            Err(WalletError::UnsupportedOperation("not used in this test".to_string()))
        }

        async fn sign_text(
            &self,
            _wallet_index: u32,
            _text: &str,
            _chain_id: WalletManagerChainId,
        ) -> Result<Signature, WalletError> {
            Err(WalletError::UnsupportedOperation("not used in this test".to_string()))
        }

        async fn sign_typed_data(
            &self,
            _wallet_index: u32,
            _typed_data: &TypedData,
            _chain_id: WalletManagerChainId,
        ) -> Result<Signature, WalletError> {
            Err(WalletError::UnsupportedOperation("not used in this test".to_string()))
        }

        fn supports_blobs(&self) -> bool {
            true
        }
    }

    struct UnusedGasEstimator;

    #[async_trait]
    impl BaseGasFeeEstimator for UnusedGasEstimator {
        async fn get_gas_prices(
            &self,
            _chain_id: &ChainId,
        ) -> Result<GasEstimatorResult, GasEstimatorError> {
            unreachable!("clone_wallet does not estimate gas")
        }

        fn is_chain_supported(&self, _chain_id: &ChainId) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn clone_wallet_passes_source_and_destination_chain_ids() {
        let last_create_chain = Arc::new(Mutex::new(None));
        let address = EvmAddress::zero();
        let wallet_manager = Arc::new(RecordingWalletManager {
            last_create_chain: last_create_chain.clone(),
            address,
        });

        let provider = EvmProvider {
            rpc_clients: Vec::new(),
            wallet_manager,
            gas_estimator: Arc::new(UnusedGasEstimator),
            block_gas_limit_cache: Arc::new(Mutex::new(None)),
            chain_id: ChainId::new(31337),
            name: "destination".to_string(),
            provider_urls: Vec::new(),
            blocks_every: 250,
            confirmations: 1,
            can_clone: true,
        };

        let source_relayer = Relayer {
            id: RelayerId::new(),
            name: "source".to_string(),
            chain_id: ChainId::new(1),
            cloned_from_chain_id: None,
            address,
            wallet_index: 7,
            max_gas_price: None,
            paused: false,
            eip_1559_enabled: true,
            created_at: Utc::now(),
            is_private_key: false,
        };

        let cloned_address = provider.clone_wallet(&source_relayer).await.unwrap();

        assert_eq!(cloned_address, address);
        assert_eq!(*last_create_chain.lock().await, Some((7, 1, 31337)));
    }
}
