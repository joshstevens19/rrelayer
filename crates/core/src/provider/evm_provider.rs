use crate::gas::BLOB_GAS_PER_BLOB;
use crate::provider::layer_extensions::RpcLoggingLayer;
use crate::wallet::{
    AwsKmsWalletManager, MnemonicWalletManager, PrivyWalletManager, TurnkeyWalletManager,
    WalletError, WalletManagerTrait,
};
use crate::yaml::{AwsKmsSigningProviderConfig, TurnkeySigningProviderConfig};
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
    primitives::Signature,
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
use std::time::Duration;
use thiserror::Error;
use tracing::info;

pub type RelayerProvider = Box<dyn Provider<AnyNetwork> + Send + Sync>;

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
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    pub async fn new_with_privy(
        network_setup_config: &NetworkSetupConfig,
        app_id: String,
        app_secret: String,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let privy_manager = PrivyWalletManager::new(app_id, app_secret).await?;
        let wallet_manager = Arc::new(privy_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    pub async fn new_with_aws_kms(
        network_setup_config: &NetworkSetupConfig,
        aws_kms_config: AwsKmsSigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(AwsKmsWalletManager::new(aws_kms_config));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    pub async fn new_with_turnkey(
        network_setup_config: &NetworkSetupConfig,
        turnkey_config: TurnkeySigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    ) -> Result<Self, EvmProviderNewError> {
        let turnkey_manager = TurnkeyWalletManager::new(turnkey_config).await?;
        let wallet_manager = Arc::new(turnkey_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

    async fn new_internal(
        network_setup_config: &NetworkSetupConfig,
        wallet_manager: Arc<dyn WalletManagerTrait>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
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
            chain_id,
            name: network_setup_config.name.to_string(),
            provider_urls: network_setup_config.provider_urls.to_owned(),
            confirmations: network_setup_config.confirmations.unwrap_or(12),
        })
    }

    pub fn rpc_client(&self) -> Arc<RelayerProvider> {
        let mut rng = thread_rng();
        let index = rng.gen_range(0..self.rpc_clients.len());
        self.rpc_clients[index].clone()
    }

    pub async fn create_wallet(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.create_wallet(wallet_index, &self.chain_id).await
    }

    pub async fn get_address(&self, wallet_index: u32) -> Result<EvmAddress, WalletError> {
        self.wallet_manager.get_address(wallet_index, &self.chain_id).await
    }

    pub async fn get_receipt(
        &self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<AnyTransactionReceipt>, RpcError<TransportErrorKind>> {
        let receipt =
            self.rpc_client().get_transaction_receipt(transaction_hash.into_alloy_hash()).await?;

        Ok(receipt)
    }

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

    pub async fn sign_transaction(
        &self,
        wallet_index: &u32,
        transaction: &TypedTransaction,
    ) -> Result<Signature, WalletError> {
        self.wallet_manager.sign_transaction(*wallet_index, transaction, &self.chain_id).await
    }

    pub async fn sign_text(
        &self,
        wallet_index: &u32,
        text: &str,
    ) -> Result<Signature, WalletError> {
        self.wallet_manager.sign_text(*wallet_index, text).await
    }

    pub async fn sign_typed_data(
        &self,
        wallet_index: &u32,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        self.wallet_manager.sign_typed_data(*wallet_index, typed_data).await
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
