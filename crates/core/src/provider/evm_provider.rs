use std::sync::Arc;

use alloy::{
    consensus::TypedTransaction,
    dyn_abi::eip712::TypedData,
    eips::{BlockId, BlockNumberOrTag},
    network::{
        primitives::BlockTransactionsKind, Ethereum, EthereumWallet, TransactionBuilder,
        TransactionBuilderError, TxSigner,
    },
    primitives::PrimitiveSignature,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::{
        client::ClientBuilder,
        types::{TransactionReceipt, TransactionRequest},
    },
    signers::{local::LocalSignerError, Signer},
    transports::{
        http::{Client, Http},
        layers::{RetryBackoffLayer, RetryBackoffService},
        RpcError, TransportErrorKind,
    },
};
use rand::{thread_rng, Rng};
use reqwest::Url;
use thiserror::Error;
use tracing::info;

use super::wallet_manager::WalletManager;
use crate::{
    gas::{
        blob_gas_oracle::{BlobGasEstimatorResult, BlobGasPriceResult},
        fee_estimator::{
            base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult},
            fallback::FallbackGasFeeEstimator,
        },
        types::GasLimit,
    },
    network::types::ChainId,
    shared::common_types::{EvmAddress, WalletOrProviderError},
    transaction::types::{TransactionHash, TransactionNonce},
};

pub type RelayerProvider = RootProvider<RetryBackoffService<Http<Client>>>;

#[derive(Clone)]
pub struct EvmProvider {
    rpc_clients: Vec<Arc<RelayerProvider>>,
    wallet_manager: Arc<WalletManager>,
    gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
    pub chain_id: ChainId,
    pub name: String,
    pub provider_urls: Vec<String>,
    /// this is in seconds
    pub blocks_every: u64,
}

pub async fn calculate_block_time_difference(
    provider: &RelayerProvider,
) -> Result<u64, RpcError<TransportErrorKind>> {
    let latest_block_number = provider.get_block_number().await?;

    // Ensure there's no underflow if not enough blocks to check set to 2 seconds
    if latest_block_number <= 13 {
        info!("Not enough blocks to calculate block time difference, setting to 2 seconds");
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

    let provider = ProviderBuilder::new().on_client(client);

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

    #[error("{0}")]
    ProviderError(RpcError<TransportErrorKind>),
}

impl EvmProvider {
    pub async fn new(
        provider_urls: &[String],
        name: &str,
        mnemonic: &str,
        gas_estimator: Option<Arc<dyn BaseGasFeeEstimator + Send + Sync>>,
    ) -> Result<Self, EvmProviderNewError> {
        // get the first one to avoid calling chainId a lot
        let provider = create_retry_client(&provider_urls[0]).map_err(|e| {
            EvmProviderNewError::HttpProviderCantBeCreated(provider_urls[0].clone(), e.to_string())
        })?;

        let chain_id = ChainId::new(
            provider.get_chain_id().await.map_err(EvmProviderNewError::ProviderError)?,
        );

        let mut providers: Vec<Arc<RelayerProvider>> = vec![];
        providers.push(provider.clone());
        for url in provider_urls.iter().skip(1) {
            providers.push(create_retry_client(url).map_err(|e| {
                EvmProviderNewError::HttpProviderCantBeCreated(url.clone(), e.to_string())
            })?);
        }

        Ok(EvmProvider {
            blocks_every: calculate_block_time_difference(&provider)
                .await
                .map_err(EvmProviderNewError::ProviderError)?,
            rpc_clients: providers,
            wallet_manager: Arc::new(WalletManager::new(mnemonic)),
            gas_estimator: gas_estimator
                .unwrap_or_else(|| Arc::new(FallbackGasFeeEstimator::new(provider.clone()))),
            chain_id,
            name: name.to_string(),
            provider_urls: provider_urls.to_owned(),
        })
    }

    pub fn rpc_client(&self) -> Arc<RelayerProvider> {
        let mut rng = thread_rng();
        let index = rng.gen_range(0..self.rpc_clients.len());
        self.rpc_clients[index].clone()
    }

    pub async fn get_address(&self, wallet_index: u32) -> Result<EvmAddress, LocalSignerError> {
        let wallet = self.wallet_manager.get_wallet(wallet_index, &self.chain_id).await?;

        Ok(EvmAddress::new(wallet.address()))
    }

    pub async fn get_receipt(
        &self,
        transaction_hash: &TransactionHash,
    ) -> Result<Option<TransactionReceipt>, RpcError<TransportErrorKind>> {
        let receipt =
            self.rpc_client().get_transaction_receipt(transaction_hash.into_alloy_hash()).await?;

        Ok(receipt)
    }

    pub async fn get_nonce(
        &self,
        wallet_index: &u32,
    ) -> Result<TransactionNonce, WalletOrProviderError> {
        let wallet = self
            .wallet_manager
            .get_wallet(*wallet_index, &self.chain_id)
            .await
            .map_err(WalletOrProviderError::WalletError)?;

        let nonce = self
            .rpc_client()
            .get_transaction_count(wallet.address())
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
        let local_signer = self.wallet_manager.get_wallet(*wallet_index, &self.chain_id).await?;

        let wallet = EthereumWallet::new(local_signer);

        let tx_request: TransactionRequest = transaction.into();
        let tx_envelope = tx_request.build(&wallet).await?;

        let provider = self.rpc_client();

        let receipt = provider.send_tx_envelope(tx_envelope).await?;

        Ok(TransactionHash::from_alloy_hash(receipt.tx_hash()))
    }

    pub async fn sign_transaction(
        &self,
        wallet_index: &u32,
        transaction: &TypedTransaction,
    ) -> Result<PrimitiveSignature, LocalSignerError> {
        let wallet = self.wallet_manager.get_wallet(*wallet_index, &self.chain_id).await?;

        let signature = match transaction {
            TypedTransaction::Legacy(tx) => {
                let mut tx = tx.clone();
                // TODO: fix this
                wallet.sign_transaction(&mut tx).await.unwrap()
            }
            TypedTransaction::Eip2930(tx) => {
                let mut tx = tx.clone();
                // TODO: fix this
                wallet.sign_transaction(&mut tx).await.unwrap()
            }
            TypedTransaction::Eip1559(tx) => {
                let mut tx = tx.clone();
                // TODO: fix this
                wallet.sign_transaction(&mut tx).await.unwrap()
            }
            TypedTransaction::Eip4844(tx) => {
                let mut tx = tx.clone();
                // TODO: fix this
                wallet.sign_transaction(&mut tx).await.unwrap()
            }
            TypedTransaction::Eip7702(tx) => {
                let mut tx = tx.clone();
                // TODO: fix this
                wallet.sign_transaction(&mut tx).await.unwrap()
            }
        };

        Ok(signature)
    }

    pub async fn sign_text(
        &self,
        wallet_index: &u32,
        text: &String,
    ) -> Result<PrimitiveSignature, SignTextError> {
        let wallet = self.wallet_manager.get_wallet(*wallet_index, &self.chain_id).await?;

        let signature = wallet.sign_message(text.as_bytes()).await?;

        Ok(signature)
    }

    pub async fn sign_typed_data(
        &self,
        wallet_index: &u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, SignTypedDataError> {
        let wallet = self.wallet_manager.get_wallet(*wallet_index, &self.chain_id).await?;

        let signature = wallet.sign_dynamic_typed_data(typed_data).await?;

        Ok(signature)
    }

    pub async fn estimate_gas(
        &self,
        transaction: &TypedTransaction,
    ) -> Result<GasLimit, RpcError<TransportErrorKind>> {
        let request: TransactionRequest = transaction.clone().into();

        let result = self.rpc_client().estimate_gas(&request).await?;

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
