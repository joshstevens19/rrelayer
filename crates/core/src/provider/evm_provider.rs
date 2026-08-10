use crate::gas::BLOB_GAS_PER_BLOB;
use crate::provider::endpoint_health::{
    spawn_endpoint_prober, EndpointClient, EndpointHealth, EndpointSelector,
};
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
use reqwest::Url;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub type RelayerProvider = Box<dyn Provider<AnyNetwork> + Send + Sync>;

const BLOCK_GAS_LIMIT_CACHE_TTL: Duration = Duration::from_secs(600);

#[derive(Clone)]
struct BlockGasLimitCache {
    gas_limit: GasLimit,
    fetched_at: Instant,
}

#[derive(Clone)]
pub struct EvmProvider {
    endpoints: EndpointSelector,
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

/// Rate-limit retries stay in the single digits ON PURPOSE: with the previous 5000
/// the retry layer could sit on a dying endpoint for ages and errors never surfaced
/// to health-aware selection. Persistence across endpoints is selection's job now.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

fn build_relayer_provider(
    rpc_url: &str,
    logging_layer: RpcLoggingLayer,
) -> Result<Arc<RelayerProvider>, RetryClientError> {
    let rpc_url = Url::parse(rpc_url).map_err(|e| {
        RetryClientError::HttpProviderCantBeCreated(rpc_url.to_string(), e.to_string())
    })?;

    let client_with_auth = Client::builder().timeout(Duration::from_secs(15)).build()?;

    let http = Http::with_client(client_with_auth, rpc_url);
    let retry_layer = RetryBackoffLayer::new(MAX_RATE_LIMIT_RETRIES, 1000, 660);
    let rpc_client =
        RpcClient::builder().layer(retry_layer).layer(logging_layer).transport(http, false);
    let provider =
        ProviderBuilder::new().network::<AnyNetwork>().connect_client(rpc_client.clone());

    Ok(Arc::new(Box::new(provider)))
}

pub async fn create_retry_client(rpc_url: &str) -> Result<Arc<RelayerProvider>, RetryClientError> {
    build_relayer_provider(rpc_url, RpcLoggingLayer::new(rpc_url.to_string()))
}

/// Builds one [`EndpointClient`] per url, each with its own health state fed by the
/// logging layer. No endpoint is contacted here - verification happens at boot in
/// `new_internal` and afterwards by the prober.
pub(crate) fn connect_endpoints(
    provider_urls: &[String],
) -> Result<EndpointSelector, RetryClientError> {
    let mut endpoints = Vec::with_capacity(provider_urls.len());

    for url in provider_urls {
        let health = Arc::new(EndpointHealth::new());
        let logging_layer = RpcLoggingLayer::new(url.to_string()).with_health(health.clone());
        let client = build_relayer_provider(url, logging_layer)?;
        endpoints.push(EndpointClient { client, url: url.clone(), health });
    }

    Ok(EndpointSelector::from_endpoints(endpoints))
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

    #[error("provider url {url} for network {network} serves chain id {got} but the config expects {expected} - refusing to start with a wrong-chain endpoint")]
    ChainIdMismatch { network: String, url: String, expected: u64, got: u64 },

    #[error("no provider url for network {0} responded at boot - cannot verify the chain or measure block time")]
    NoResponsiveProviderUrls(String),
}

/// Boot policy for per-url chain verification. Serving the WRONG chain is a
/// configuration bug and fails boot naming the url (silently relaying funds-bearing
/// transactions into another chain's mempool is never acceptable). An endpoint that
/// is merely UNREACHABLE at boot only warns and is reported not-verified - it must
/// not take traffic yet but may recover later, so a dead endpoint (even url[0])
/// cannot kill the whole server while healthy ones exist.
///
/// Returns one flag per url: true when the endpoint answered with the expected
/// chain id.
fn verify_endpoint_chain_ids(
    network: &str,
    checks: &[(String, Result<u64, String>)],
    expected: ChainId,
) -> Result<Vec<bool>, EvmProviderNewError> {
    let mut verified = Vec::with_capacity(checks.len());

    for (url, result) in checks {
        match result {
            Ok(got) if *got == expected.u64() => verified.push(true),
            Ok(got) => {
                return Err(EvmProviderNewError::ChainIdMismatch {
                    network: network.to_string(),
                    url: url.clone(),
                    expected: expected.u64(),
                    got: *got,
                });
            }
            Err(error) => {
                warn!(
                    "Provider url {} for network {} is unreachable at boot ({}) - continuing without it until it recovers",
                    url, network, error
                );
                verified.push(false);
            }
        }
    }

    Ok(verified)
}

impl EvmProvider {
    pub async fn new_with_mnemonic(
        network_setup_config: &NetworkSetupConfig,
        mnemonic: &str,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(MnemonicWalletManager::new(mnemonic));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    pub async fn new_with_privy(
        network_setup_config: &NetworkSetupConfig,
        app_id: String,
        app_secret: String,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let privy_manager = PrivyWalletManager::new(app_id, app_secret).await?;
        let wallet_manager = Arc::new(privy_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    pub async fn new_with_aws_kms(
        network_setup_config: &NetworkSetupConfig,
        aws_kms_config: AwsKmsSigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(AwsKmsWalletManager::new(aws_kms_config));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    pub async fn new_with_turnkey(
        network_setup_config: &NetworkSetupConfig,
        turnkey_config: TurnkeySigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let turnkey_manager = TurnkeyWalletManager::new(turnkey_config).await?;
        let wallet_manager = Arc::new(turnkey_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    pub async fn new_with_private_keys(
        network_setup_config: &NetworkSetupConfig,
        private_keys: Vec<String>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(PrivateKeyWalletManager::new(private_keys));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, false)
            .await
    }

    pub async fn new_with_pkcs11(
        network_setup_config: &NetworkSetupConfig,
        pkcs11_config: Pkcs11SigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let wallet_manager = Arc::new(Pkcs11WalletManager::new(pkcs11_config)?);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    pub async fn new_with_fireblocks(
        network_setup_config: &NetworkSetupConfig,
        fireblocks_config: FireblocksSigningProviderConfig,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let fireblocks_manager = FireblocksWalletManager::new(fireblocks_config).await?;
        let wallet_manager = Arc::new(fireblocks_manager);
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, false)
            .await
    }

    pub async fn new_with_composite(
        network_setup_config: &NetworkSetupConfig,
        primary_manager: Arc<dyn WalletManagerTrait>,
        private_keys: Option<Vec<String>>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
    ) -> Result<Self, EvmProviderNewError> {
        let private_key_manager = private_keys.map(|private_keys| {
            Arc::new(PrivateKeyWalletManager::new(private_keys)) as Arc<dyn WalletManagerTrait>
        });

        let wallet_manager =
            Arc::new(CompositeWalletManager::new(primary_manager, private_key_manager));
        Self::new_internal(network_setup_config, wallet_manager, gas_estimator, endpoints, true)
            .await
    }

    async fn new_internal(
        network_setup_config: &NetworkSetupConfig,
        wallet_manager: Arc<dyn WalletManagerTrait>,
        gas_estimator: Arc<dyn BaseGasFeeEstimator + Send + Sync>,
        endpoints: EndpointSelector,
        can_clone: bool,
    ) -> Result<Self, EvmProviderNewError> {
        // Verify EVERY url serves the configured chain before it can take traffic -
        // wrong-chain fails boot, unreachable-at-boot only warns (see
        // verify_endpoint_chain_ids for the policy)
        let mut chain_checks: Vec<(String, Result<u64, String>)> =
            Vec::with_capacity(endpoints.endpoints().len());
        for endpoint in endpoints.endpoints() {
            chain_checks.push((
                endpoint.url.clone(),
                endpoint.client.get_chain_id().await.map_err(|e| e.to_string()),
            ));
        }
        let verified = verify_endpoint_chain_ids(
            &network_setup_config.name,
            &chain_checks,
            network_setup_config.chain_id,
        )?;

        // Verified endpoints enter rotation; unreachable ones stay unverified and
        // therefore unselectable until the prober verifies their chain id
        for (endpoint, verified) in endpoints.endpoints().iter().zip(verified.iter()) {
            if *verified {
                endpoint.health.mark_chain_verified();
            }
        }

        // Measure block time on the first VERIFIED endpoint that answers - pinning
        // this to url[0] would make a dead primary kill boot even with healthy
        // fallbacks configured
        let mut blocks_every = None;
        for (index, endpoint) in endpoints.endpoints().iter().enumerate() {
            if !verified[index] {
                continue;
            }
            match calculate_block_time_difference(&endpoint.client).await {
                Ok(block_time_ms) => {
                    blocks_every = Some(block_time_ms);
                    break;
                }
                Err(error) => {
                    warn!(
                        "Could not measure block time on provider url {} for network {}: {} - trying the next url",
                        endpoint.url, network_setup_config.name, error
                    );
                }
            }
        }
        let Some(blocks_every) = blocks_every else {
            return Err(EvmProviderNewError::NoResponsiveProviderUrls(
                network_setup_config.name.to_string(),
            ));
        };

        // Background tip probe: lag detection plus the half-open recovery signal for
        // cooled-down or boot-unreachable endpoints
        spawn_endpoint_prober(
            endpoints.clone(),
            network_setup_config.name.to_string(),
            network_setup_config.chain_id,
        );

        Ok(EvmProvider {
            blocks_every,
            endpoints,
            wallet_manager,
            gas_estimator,
            block_gas_limit_cache: Arc::new(Mutex::new(None)),
            // The chain id comes from the CONFIG (each url was verified against it
            // above) - never from whatever url[0] happens to answer
            chain_id: network_setup_config.chain_id,
            name: network_setup_config.name.to_string(),
            provider_urls: network_setup_config.provider_urls.to_owned(),
            confirmations: network_setup_config.confirmations.unwrap_or(12),
            can_clone,
        })
    }

    /// Health-aware client selection: endpoints in cooldown or lagging the best tip
    /// are skipped and the rest weighted by their record; when everything looks
    /// unhealthy the whole enabled set is used so a client is always returned.
    pub fn rpc_client(&self) -> Arc<RelayerProvider> {
        self.endpoints.client()
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

    /// Derives the address the CURRENTLY configured signing provider produces
    /// for a relayer's wallet index — the address signing would actually use.
    /// Embedders verify imported relayers against this (a relayer created
    /// under a different signing provider derives a different address).
    pub async fn derived_address(&self, relayer: &Relayer) -> Result<EvmAddress, WalletError> {
        self.wallet_manager
            .get_address(relayer.wallet_index(), relayer.wallet_manager_chain_id())
            .await
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

    /// The MINED transaction count (`latest` tag) - unlike [`Self::get_nonce_from_address`]
    /// this never counts transactions still sitting in the node's mempool, so it is the
    /// right comparison for "was this nonce consumed on chain": a broadcast-but-unmined
    /// holder must still be replaceable/cancellable at its nonce.
    pub async fn get_mined_nonce_from_address(
        &self,
        address: &EvmAddress,
    ) -> Result<TransactionNonce, RpcError<TransportErrorKind>> {
        let nonce = self
            .rpc_client()
            .get_transaction_count(address.into_address())
            .block_id(BlockId::Number(BlockNumberOrTag::Latest))
            .await?;

        Ok(TransactionNonce::new(nonce))
    }

    pub async fn send_transaction(
        &self,
        relayer: &Relayer,
        transaction: TypedTransaction,
    ) -> Result<TransactionHash, SendTransactionError> {
        let signature = self
            .sign_transaction(relayer, &transaction)
            .await
            .map_err(|e| SendTransactionError::InternalError(e.to_string()))?;

        self.send_signed_transaction(transaction, signature).await
    }

    pub async fn send_signed_transaction(
        &self,
        transaction: TypedTransaction,
        signature: Signature,
    ) -> Result<TransactionHash, SendTransactionError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relayer::RelayerId;
    use crate::wallet::WalletManagerChainId;
    use async_trait::async_trait;
    use chrono::Utc;
    use tokio::sync::Mutex;

    struct RecordingWalletManager {
        last_create_chain: Arc<Mutex<Option<(u64, u64)>>>,
        address: EvmAddress,
    }

    #[async_trait]
    impl WalletManagerTrait for RecordingWalletManager {
        async fn create_wallet(
            &self,
            _wallet_index: u32,
            chain_id: WalletManagerChainId,
        ) -> Result<EvmAddress, WalletError> {
            match chain_id {
                WalletManagerChainId::Cloned(chain) => {
                    let mut last_create_chain = self.last_create_chain.lock().await;
                    *last_create_chain = Some((chain.cloned_from.u64(), chain.cloned_to.u64()));
                }
                WalletManagerChainId::ChainId(chain_id) => {
                    let mut last_create_chain = self.last_create_chain.lock().await;
                    *last_create_chain = Some((chain_id.u64(), chain_id.u64()));
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
            endpoints: EndpointSelector::from_endpoints(Vec::new()),
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
        assert_eq!(*last_create_chain.lock().await, Some((1, 31337)));
    }

    #[test]
    fn wrong_chain_url_fails_boot_naming_url_and_chain_ids() {
        let checks = vec![
            ("https://one.example".to_string(), Ok(1)),
            ("https://two.example".to_string(), Ok(137)),
        ];

        let result = verify_endpoint_chain_ids("ethereum", &checks, ChainId::new(1));

        match result {
            Err(EvmProviderNewError::ChainIdMismatch { network, url, expected, got }) => {
                assert_eq!(network, "ethereum");
                assert_eq!(url, "https://two.example");
                assert_eq!(expected, 1);
                assert_eq!(got, 137);
            }
            other => panic!("expected a chain id mismatch, got {other:?}"),
        }
    }

    #[test]
    fn unreachable_url_at_boot_warns_instead_of_failing() {
        let checks = vec![
            ("https://dead.example".to_string(), Err("connection refused".to_string())),
            ("https://alive.example".to_string(), Ok(1)),
        ];

        let verified = verify_endpoint_chain_ids("ethereum", &checks, ChainId::new(1))
            .expect("an unreachable endpoint must not fail boot");

        assert_eq!(verified, vec![false, true]);
    }

    #[test]
    fn all_matching_urls_verify() {
        let checks = vec![
            ("https://one.example".to_string(), Ok(31337)),
            ("https://two.example".to_string(), Ok(31337)),
        ];

        let verified = verify_endpoint_chain_ids("anvil", &checks, ChainId::new(31337))
            .expect("matching endpoints must verify");

        assert_eq!(verified, vec![true, true]);
    }

    #[test]
    fn wrong_chain_takes_precedence_over_unreachable_urls() {
        // A wrong-chain url is a config bug even when other urls are down
        let checks = vec![
            ("https://dead.example".to_string(), Err("timeout".to_string())),
            ("https://wrong.example".to_string(), Ok(10)),
        ];

        let result = verify_endpoint_chain_ids("ethereum", &checks, ChainId::new(1));

        assert!(matches!(result, Err(EvmProviderNewError::ChainIdMismatch { .. })));
    }
}
