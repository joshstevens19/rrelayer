use std::sync::Arc;

use alloy::consensus::{SignableTransaction, TxEnvelope};
use alloy::network::{AnyNetwork, AnyTransactionReceipt, AnyTxEnvelope};
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

use crate::wallet::{MnemonicWalletManager, PrivyWalletManager, WalletError, WalletManagerTrait};
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
    /// this is in seconds
    pub blocks_every: u64,
    pub confirmations: u64,
}

pub async fn calculate_block_time_difference(
    provider: &RelayerProvider,
) -> Result<u64, RpcError<TransportErrorKind>> {
    let latest_block_number = provider.get_block_number().await?;

    // Ensure there's no underflow if not enough blocks to check set to 2 seconds
    if latest_block_number <= 13 {
        rrelayer_info!(
            "Not enough blocks to calculate block time difference, setting to 2 seconds"
        );
        return Ok(2);
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

    Ok(latest.header.timestamp - earliest.header.timestamp)
}

#[derive(Error, Debug)]
pub enum RetryClientError {
    #[error("http provider cant be created for {0}: {1}")]
    HttpProviderCantBeCreated(String, String),
}

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
pub enum SignTextError {
    #[error("Wallet error: {0}")]
    WalletError(#[from] LocalSignerError),

    #[error("Signing message failed: {0}")]
    SignMessageError(#[from] alloy::signers::Error),
}

#[derive(Error, Debug)]
pub enum SignTypedDataError {
    #[error("Wallet error: {0}")]
    WalletError(#[from] LocalSignerError),

    #[error("Signing message failed: {0}")]
    SignMessageError(#[from] alloy::signers::Error),
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
        let privy_manager = PrivyWalletManager::new(app_id, app_secret)
            .await
            .map_err(|e| EvmProviderNewError::WalletManagerError(e.to_string()))?;
        let wallet_manager = Arc::new(privy_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator).await
    }

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
            .get_transaction_count(address.into())
            .block_id(BlockId::Number(BlockNumberOrTag::Pending))
            .await
            .map_err(WalletOrProviderError::ProviderError)?;

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
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_transaction(*wallet_index, transaction, &self.chain_id).await
    }

    pub async fn sign_text(
        &self,
        wallet_index: &u32,
        text: &String,
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_text(*wallet_index, text).await
    }

    pub async fn sign_typed_data(
        &self,
        wallet_index: &u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        self.wallet_manager.sign_typed_data(*wallet_index, typed_data).await
    }

    pub async fn estimate_gas(
        &self,
        transaction: &TypedTransaction,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        let request: TransactionRequest = transaction.clone().into();

        let request_with_other = WithOtherFields::new(request);

        let result = self.rpc_client().estimate_gas(&request_with_other).await?;

        Ok(GasLimit::new(result as u128))
    }

    pub async fn calculate_gas_price(&self) -> Result<GasEstimatorResult, GasEstimatorError> {
        self.gas_estimator.get_gas_prices(&self.chain_id).await
    }

    pub fn supports_blob_transactions(&self) -> bool {
        // Ethereum mainnet and testnet chain IDs that support blobs
        matches!(
            self.chain_id.u64(),
            1 |      // Ethereum Mainnet
           17000 |  // Holesky Testnet
           11155111 // Sepolia Testnet
        )
    }

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
        let medium_multiplier = 1.0;
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
