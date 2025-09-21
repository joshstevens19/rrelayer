use crate::shared::utils::{format_token_amount, format_wei_to_eth};
use crate::{
    network::types::ChainId,
    postgres::{PostgresClient, PostgresError},
    provider::EvmProvider,
    relayer::types::Relayer,
    safe_proxy::SafeProxyManager,
    shared::common_types::{EvmAddress, PagingContext},
    yaml::{AutomaticTopUpConfig, Erc20TokenConfig, NativeTokenConfig, TopUpTargetAddresses},
    SetupConfig,
};
use alloy::consensus::TxLegacy;
use alloy::consensus::TypedTransaction;
use alloy::primitives::U256;
use alloy::providers::Provider;
use alloy::rpc::types::serde_helpers::WithOtherFields;
use alloy::sol;
use alloy::sol_types::SolCall;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::{interval, Interval};
use tracing::{error, info, warn};

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

/// Automatic top-up background task for managing relayer address balances.
///
/// This struct manages the automatic top-up functionality that monitors relayer
/// addresses and ensures they maintain minimum balance thresholds by automatically
/// transferring funds from configured source addresses.
///
/// # Key Features
/// - Periodic balance monitoring of relayer addresses
/// - Automatic fund transfers when balances fall below thresholds
/// - Support for multiple blockchain networks
/// - Configurable refresh and check intervals
/// - Intelligent wallet index resolution for transactions
///
/// # Configuration
/// The task behavior is controlled through the `SetupConfig` which contains
/// network-specific `AutomaticTopUpConfig` settings including:
/// - Source address for funds (`from_address`)
/// - Target addresses (all relayers or specific list)
/// - Minimum balance threshold (`min_balance`)
/// - Top-up amount (`top_up_amount`)
pub struct AutomaticTopUpTask {
    postgres_client: Arc<PostgresClient>,
    providers: Arc<Vec<EvmProvider>>,
    config: SetupConfig,
    safe_proxy_manager: Option<SafeProxyManager>,
    relayer_cache: HashMap<ChainId, Vec<Relayer>>,
    relayer_refresh_interval: Interval,
    top_up_check_interval: Interval,
}

impl AutomaticTopUpTask {
    /// Creates a new AutomaticTopUpTask instance.
    ///
    /// # Arguments
    /// * `postgres_client` - Database client for querying relayer information
    /// * `providers` - Collection of EVM providers for different chains
    /// * `config` - Setup configuration containing network and top-up settings
    pub fn new(
        postgres_client: Arc<PostgresClient>,
        providers: Arc<Vec<EvmProvider>>,
        config: SetupConfig,
    ) -> Self {
        // Initialize safe proxy manager if any safe proxy configs exist
        let safe_proxy_manager =
            config.safe_proxy.as_ref().map(|configs| SafeProxyManager::new(configs.clone()));

        Self {
            postgres_client,
            providers,
            config,
            safe_proxy_manager,
            relayer_cache: HashMap::new(),
            relayer_refresh_interval: interval(Duration::from_secs(30)),
            top_up_check_interval: interval(Duration::from_secs(30)),
        }
    }

    /// Main execution loop for the automatic top-up task.
    ///
    /// This method runs indefinitely, periodically refreshing the relayer cache
    /// and checking addresses that need top-up based on configured intervals.
    pub async fn run(&mut self) {
        info!("Starting automatic top-up background task");

        self.refresh_relayer_cache().await;

        loop {
            tokio::select! {
                _ = self.relayer_refresh_interval.tick() => {
                    self.refresh_relayer_cache().await;
                }
                _ = self.top_up_check_interval.tick() => {
                    self.check_and_top_up_addresses().await;
                }
            }
        }
    }

    /// Refreshes the internal cache of relayers for all configured networks.
    ///
    /// This method queries the database to get the latest relayer information
    /// for each network that has automatic top-up configured.
    async fn refresh_relayer_cache(&mut self) {
        for network_config in &self.config.networks {
            if let Some(_automatic_top_up) = &network_config.automatic_top_up {
                info!("Refreshing relayer cache for {}", network_config.name);

                let chain_id = match network_config.get_chain_id().await {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Failed to get chain ID for network {}: {}", network_config.name, e);
                        continue;
                    }
                };

                match self.get_all_relayers_for_chain(&chain_id).await {
                    Ok(relayers) => {
                        info!("Cached {} relayers for chain {}", relayers.len(), chain_id);
                        self.relayer_cache.insert(chain_id, relayers);
                    }
                    Err(e) => {
                        error!("Failed to refresh relayer cache for chain {}: {}", chain_id, e);
                    }
                }
            }
        }
    }

    /// Retrieves all relayers for a specific chain from the database.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to query relayers for
    ///
    /// # Returns
    /// * `Ok(Vec<Relayer>)` - List of all relayers for the chain
    /// * `Err(PostgresError)` - Database error if query fails
    async fn get_all_relayers_for_chain(
        &self,
        chain_id: &ChainId,
    ) -> Result<Vec<Relayer>, PostgresError> {
        let mut all_relayers = Vec::new();
        let mut offset = 0;
        let limit = 100;

        loop {
            let paging_context = PagingContext::new(limit, offset);
            let result =
                self.postgres_client.get_relayers_for_chain(chain_id, &paging_context).await?;

            let relayer_count = result.items.len();
            all_relayers.extend(result.items);

            if relayer_count < limit as usize {
                break;
            }

            offset += limit;
        }

        Ok(all_relayers)
    }

    /// Checks all configured addresses and performs top-ups where needed.
    ///
    /// This method iterates through all networks with automatic top-up configured,
    /// checks balances against minimum thresholds, and initiates top-up transactions
    /// for addresses that fall below the configured minimum balance.
    async fn check_and_top_up_addresses(&self) {
        for network_config in &self.config.networks {
            if let Some(automatic_top_up) = &network_config.automatic_top_up {
                info!("Checking addresses for top-up on {}", network_config.name);

                let chain_id = match network_config.get_chain_id().await {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Failed to get chain ID for network {}: {}", network_config.name, e);
                        continue;
                    }
                };

                let provider = match self.get_provider_for_chain(&chain_id) {
                    Some(p) => p,
                    None => {
                        warn!("No provider found for chain {}. Skipping top-up checks.", chain_id);
                        continue;
                    }
                };

                self.process_top_up_config(&chain_id, provider, automatic_top_up).await;
            }
        }
    }

    /// Processes a single automatic top-up configuration for a specific chain.
    async fn process_top_up_config(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        config: &AutomaticTopUpConfig,
    ) {
        info!("Processing top-up config for chain {} from {}", chain_id, config.from_address);

        let target_addresses = match self
            .resolve_target_addresses(chain_id, &config.targets, &config.from_address)
            .await
        {
            Ok(addresses) => addresses,
            Err(e) => {
                error!("Failed to resolve target addresses for chain {}: {}", chain_id, e);
                return;
            }
        };

        if target_addresses.is_empty() {
            info!("No target addresses found for top-up on chain {}", chain_id);
            return;
        }

        if let Some(native_config) = &config.native {
            if native_config.enabled {
                info!("Processing native token top-ups for {} addresses", target_addresses.len());
                self.process_native_token_top_ups(
                    chain_id,
                    provider,
                    &config.from_address,
                    &target_addresses,
                    native_config,
                    config,
                )
                .await;
            } else {
                info!("Native token top-ups disabled for chain {}", chain_id);
            }
        }

        if let Some(erc20_tokens) = &config.erc20_tokens {
            for (index, token_config) in erc20_tokens.iter().enumerate() {
                info!(
                    "Processing ERC-20 token top-ups for token {} ({}/{}) on {} addresses",
                    token_config.address,
                    index + 1,
                    erc20_tokens.len(),
                    target_addresses.len()
                );
                self.process_erc20_token_top_ups(
                    chain_id,
                    provider,
                    &config.from_address,
                    &target_addresses,
                    token_config,
                    config,
                )
                .await;
            }
        }

        if config.native.is_none() && config.erc20_tokens.is_none() {
            warn!(
                "No token configurations found for chain {}. Please configure either native or erc20_tokens.",
                chain_id
            );
        }
    }

    /// Processes native token (ETH) top-ups for target addresses.
    async fn process_native_token_top_ups(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        target_addresses: &[EvmAddress],
        native_config: &NativeTokenConfig,
        config: &AutomaticTopUpConfig,
    ) {
        let mut addresses_needing_top_up = Vec::new();

        for address in target_addresses {
            match provider.rpc_client().get_balance((*address).into()).await {
                Ok(balance) => {
                    if balance < native_config.min_balance {
                        info!(
                            "Address {} native balance ({} ETH) is below minimum ({} ETH), needs top-up",
                            address,
                            format_wei_to_eth(&balance),
                            format_wei_to_eth(&native_config.min_balance)
                        );
                        addresses_needing_top_up.push(*address);
                    }
                }
                Err(e) => {
                    warn!("Failed to check native balance for address {}: {}", address, e);
                }
            }
        }

        if addresses_needing_top_up.is_empty() {
            info!(
                "All {} addresses have sufficient native token balance on chain {}",
                target_addresses.len(),
                chain_id
            );
            return;
        }

        info!(
            "{} out of {} addresses need native token top-up on chain {}",
            addresses_needing_top_up.len(),
            target_addresses.len(),
            chain_id
        );

        match self.check_native_from_address_balance(provider, from_address, native_config).await {
            Ok(sufficient) => {
                if !sufficient {
                    warn!(
                        "From address {} has insufficient native balance for top-ups on chain {}. Skipping {} addresses that need top-up.",
                        from_address, chain_id, addresses_needing_top_up.len()
                    );
                    return;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to check from_address {} native balance on chain {}: {}. Proceeding with caution.",
                    from_address, chain_id, e
                );
            }
        }

        for address in addresses_needing_top_up {
            match self
                .send_native_top_up_transaction(
                    chain_id,
                    provider,
                    from_address,
                    &address,
                    native_config,
                    config,
                )
                .await
            {
                Ok(tx_hash) => {
                    info!(
                        "Topped up address {} with {} ETH (native). Transaction: {}",
                        address,
                        format_wei_to_eth(&native_config.top_up_amount),
                        tx_hash
                    );
                }
                Err(e) => {
                    warn!("Failed to send native top-up to address {}: {}", address, e);
                }
            }
        }
    }

    /// Processes ERC-20 token top-ups for target addresses.
    async fn process_erc20_token_top_ups(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        target_addresses: &[EvmAddress],
        token_config: &Erc20TokenConfig,
        config: &AutomaticTopUpConfig,
    ) {
        let mut addresses_needing_top_up = Vec::new();

        // Check ERC-20 token balances for all target addresses
        for address in target_addresses {
            match self.get_erc20_balance(provider, &token_config.address, address).await {
                Ok(balance) => {
                    if balance < token_config.min_balance {
                        info!(
                            "Address {} ERC-20 balance ({}) is below minimum ({}) for token {}, needs top-up",
                            address,
                            format_token_amount(&balance),
                            format_token_amount(&token_config.min_balance),
                            token_config.address
                        );
                        addresses_needing_top_up.push(*address);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to check ERC-20 balance for address {} and token {}: {}",
                        address, token_config.address, e
                    );
                }
            }
        }

        if addresses_needing_top_up.is_empty() {
            info!(
                "All {} addresses have sufficient ERC-20 token balance for token {} on chain {}",
                target_addresses.len(),
                token_config.address,
                chain_id
            );
            return;
        }

        info!(
            "{} out of {} addresses need ERC-20 top-up for token {} on chain {}",
            addresses_needing_top_up.len(),
            target_addresses.len(),
            token_config.address,
            chain_id
        );

        match self.check_erc20_from_address_balance(provider, from_address, token_config).await {
            Ok(sufficient) => {
                if !sufficient {
                    warn!(
                        "From address {} has insufficient ERC-20 token balance for top-ups on chain {}. Skipping {} addresses that need top-up.",
                        from_address, chain_id, addresses_needing_top_up.len()
                    );
                    return;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to check from_address {} ERC-20 token balance on chain {}: {}. Proceeding with caution.",
                    from_address, chain_id, e
                );
            }
        }

        // Send ERC-20 token top-ups
        for address in addresses_needing_top_up {
            match self
                .send_erc20_top_up_transaction(
                    chain_id,
                    provider,
                    from_address,
                    &address,
                    token_config,
                    config,
                )
                .await
            {
                Ok(tx_hash) => {
                    info!(
                        "Topped up address {} with {} tokens ({}). Transaction: {}",
                        address,
                        format_token_amount(&token_config.top_up_amount),
                        token_config.address,
                        tx_hash
                    );
                }
                Err(e) => {
                    warn!("Failed to send ERC-20 top-up to address {}: {}", address, e);
                }
            }
        }
    }

    /// Sends a native token top-up transaction from one address to another.
    async fn send_native_top_up_transaction(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        target_address: &EvmAddress,
        native_config: &NativeTokenConfig,
        config: &AutomaticTopUpConfig,
    ) -> Result<String, String> {
        if from_address == target_address {
            return Err(format!(
                "Cannot send top-up transaction to self: from_address {} equals target_address {}",
                from_address, target_address
            ));
        }

        info!(
            "Sending top-up transaction: {} -> {} ({} ETH){}",
            from_address,
            target_address,
            format_wei_to_eth(&native_config.top_up_amount),
            if config.safe.is_some() { " via Safe proxy" } else { "" }
        );

        let (final_to, final_value, final_data) = if let Some(safe_address) = &config.safe {
            if let Some(ref safe_manager) = self.safe_proxy_manager {
                info!(
                    "Using Safe proxy {} for top-up transaction from {} to {}",
                    safe_address, from_address, target_address
                );

                let wallet_index = match self.find_wallet_index_for_address(chain_id, from_address)
                {
                    Some(index) => index,
                    None => {
                        return Err(format!(
                            "Cannot find wallet index for from_address {} on chain {}",
                            from_address, chain_id
                        ));
                    }
                };

                let (_safe_tx, encoded_data) = safe_manager
                    .create_safe_transaction_with_signature(
                        provider,
                        wallet_index,
                        safe_address,
                        *target_address,
                        native_config.top_up_amount,
                        alloy::primitives::Bytes::new(), // Empty data for native transfers
                    )
                    .await
                    .map_err(|e| format!("Failed to create Safe transaction: {}", e))?;

                (*safe_address, U256::ZERO, encoded_data)
            } else {
                return Err("Safe proxy address configured but SafeProxyManager not initialized"
                    .to_string());
            }
        } else {
            // Direct transaction
            (*target_address, native_config.top_up_amount, alloy::primitives::Bytes::new())
        };

        let tx = TypedTransaction::Legacy(TxLegacy {
            chain_id: Some(provider.chain_id.u64()),
            nonce: 0,
            gas_price: 20_000_000_000,
            gas_limit: if config.safe.is_some() { 300000 } else { 21000 },
            to: alloy::primitives::TxKind::Call(final_to.into()),
            value: final_value,
            input: final_data,
        });

        let wallet_index = if config.safe.is_none() {
            // For direct transactions, we need to find the wallet index
            match self.find_wallet_index_for_address(chain_id, from_address) {
                Some(index) => index,
                None => {
                    return Err(format!(
                        "Cannot find wallet index for from_address {} on chain {}",
                        from_address, chain_id
                    ));
                }
            }
        } else {
            // For Safe transactions, we use the signer's wallet (from_address)
            // This is already determined inside the Safe logic above
            match self.find_wallet_index_for_address(chain_id, from_address) {
                Some(index) => index,
                None => {
                    return Err(format!(
                        "Cannot find wallet index for Safe signer {} on chain {}",
                        from_address, chain_id
                    ));
                }
            }
        };

        match provider.send_transaction(&wallet_index, tx).await {
            Ok(tx_hash) => {
                info!("Top-up transaction sent successfully: {}", tx_hash);
                Ok(tx_hash.to_string())
            }
            Err(e) => {
                warn!(
                    "Failed to send top-up transaction from {} to {}: {}. This is non-fatal, will retry next cycle.",
                    from_address, target_address, e
                );

                match provider.rpc_client().get_balance((*from_address).into()).await {
                    Ok(from_balance) => {
                        let gas_price = provider
                            .rpc_client()
                            .get_gas_price()
                            .await
                            .unwrap_or(20_000_000_000u128);
                        let estimated_gas =
                            U256::from(if config.safe.is_some() { 300000 } else { 21000 })
                                * U256::from(gas_price);
                        let required_balance = if config.safe.is_some() {
                            estimated_gas
                        } else {
                            native_config.top_up_amount + estimated_gas
                        };

                        if from_balance < required_balance {
                            warn!(
                                "Transaction failure likely due to insufficient from_address balance. Available: {} ETH, Required: {} ETH",
                                format_wei_to_eth(&from_balance),
                                format_wei_to_eth(&required_balance)
                            );
                        }
                    }
                    Err(_) => {}
                }

                Err(format!("Transaction failed: {}", e))
            }
        }
    }

    /// Resolves target addresses based on the configured target type.
    ///
    /// # Arguments
    /// * `chain_id` - Chain ID to resolve addresses for
    /// * `targets` - Target address configuration (All relayers or specific list)
    /// * `from_address` - Source address to exclude from targets (prevent self-funding)
    ///
    /// # Returns
    /// * `Ok(Vec<EvmAddress>)` - List of resolved target addresses (excluding from_address)
    /// * `Err(String)` - Error message if resolution fails
    async fn resolve_target_addresses(
        &self,
        chain_id: &ChainId,
        targets: &TopUpTargetAddresses,
        from_address: &EvmAddress,
    ) -> Result<Vec<EvmAddress>, PostgresError> {
        let mut addresses = match targets {
            TopUpTargetAddresses::All => {
                match self.postgres_client.get_all_relayers_for_chain(chain_id).await {
                    Ok(relayers) => {
                        let addresses: Vec<EvmAddress> =
                            relayers.iter().map(|r| r.address).collect();
                        addresses
                    }
                    Err(e) => {
                        error!("Error fetching all the relayers on chainId {} - error {}", chain_id, e);
                        Vec::new()
                    }
                }
            }
            TopUpTargetAddresses::List(addresses) => addresses.clone(),
        };

        // Filter out the from_address to prevent self-funding
        let _original_count = addresses.len();
        let contains_from_address = addresses.contains(from_address);
        addresses.retain(|addr| addr != from_address);

        if contains_from_address {
            match targets {
                TopUpTargetAddresses::All => {
                    info!(
                        "Filtered out from_address {} from relayer targets on chain {} to prevent self-funding", 
                        from_address, chain_id
                    );
                }
                TopUpTargetAddresses::List(_) => {
                    info!(
                        "Filtered out from_address {} from explicitly configured targets on chain {} to prevent self-funding. \
                        Note: from_address should not be included in the target list in YAML configuration.", 
                        from_address, chain_id
                    );
                }
            }
        }

        Ok(addresses)
    }

    /// Checks if the from_address has sufficient native balance for top-up operations.
    ///
    /// # Arguments
    /// * `provider` - EVM provider to query balance
    /// * `from_address` - Address to check balance for
    /// * `native_config` - Native token configuration for amount calculations
    ///
    /// # Returns
    /// * `Ok(true)` - Address has sufficient balance
    /// * `Ok(false)` - Address has insufficient balance
    /// * `Err(String)` - Error occurred during balance check
    async fn check_native_from_address_balance(
        &self,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        native_config: &NativeTokenConfig,
    ) -> Result<bool, String> {
        let balance = provider
            .rpc_client()
            .get_balance((*from_address).into())
            .await
            .map_err(|e| format!("Failed to get from_address balance: {}", e))?;

        info!("From address {} has balance: {} ETH", from_address, format_wei_to_eth(&balance));

        let estimated_gas_cost =
            self.estimate_transaction_cost(provider).await.unwrap_or_else(|e| {
                warn!("Failed to estimate gas cost: {}. Using default estimate.", e);
                U256::from(21000u64) * U256::from(20_000_000_000u64)
            });

        let min_required_balance = native_config.top_up_amount + estimated_gas_cost;

        info!(
            "From address {} requires {} ETH (top-up: {} ETH + gas: {} ETH)",
            from_address,
            format_wei_to_eth(&min_required_balance),
            format_wei_to_eth(&native_config.top_up_amount),
            format_wei_to_eth(&estimated_gas_cost)
        );

        if balance < min_required_balance {
            warn!(
                "From address {} balance ({} ETH) is insufficient for top-up transaction. Required: {} ETH (top-up: {} ETH + gas: {} ETH)",
                from_address,
                format_wei_to_eth(&balance),
                format_wei_to_eth(&min_required_balance),
                format_wei_to_eth(&native_config.top_up_amount),
                format_wei_to_eth(&estimated_gas_cost)
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Estimates the gas cost for a standard transfer transaction.
    ///
    /// # Arguments
    /// * `provider` - EVM provider to query gas prices
    ///
    /// # Returns
    /// * `Ok(U256)` - Estimated transaction cost in wei
    /// * `Err(String)` - Error message if estimation fails
    async fn estimate_transaction_cost(&self, provider: &EvmProvider) -> Result<U256, String> {
        let gas_price = provider
            .rpc_client()
            .get_gas_price()
            .await
            .map_err(|e| format!("Failed to get gas price: {}", e))?;

        let gas_limit = U256::from(21000u64);
        let total_cost = U256::from(gas_price) * gas_limit;

        info!(
            "Estimated transaction cost: {} ETH (gas price: {} gwei, limit: {})",
            format_wei_to_eth(&total_cost),
            U256::from(gas_price) / U256::from(1_000_000_000u64),
            gas_limit
        );

        Ok(total_cost)
    }

    /// Finds the EVM provider for a specific chain ID.
    ///
    /// # Arguments
    /// * `chain_id` - Chain ID to find provider for
    ///
    /// # Returns
    /// * `Some(&EvmProvider)` - Provider found for the chain
    /// * `None` - No provider configured for this chain
    fn get_provider_for_chain(&self, chain_id: &ChainId) -> Option<&EvmProvider> {
        self.providers.iter().find(|p| &p.chain_id == chain_id)
    }

    /// Finds the wallet index for a specific address on a given chain.
    ///
    /// # Arguments
    /// * `chain_id` - Chain ID to search in
    /// * `address` - Address to find wallet index for
    ///
    /// # Returns
    /// * `Some(u32)` - Wallet index if address is found in relayer cache
    /// * `None` - Address not found in cache or chain not cached
    fn find_wallet_index_for_address(
        &self,
        chain_id: &ChainId,
        address: &EvmAddress,
    ) -> Option<u32> {
        match self.relayer_cache.get(chain_id) {
            Some(relayers) => relayers
                .iter()
                .find(|relayer| &relayer.address == address)
                .map(|relayer| relayer.wallet_index),
            None => None,
        }
    }

    /// Checks if the from_address has sufficient ERC-20 token balance for top-up operations.
    ///
    /// # Arguments
    /// * `provider` - EVM provider to query balance
    /// * `from_address` - Address to check balance for
    /// * `token_config` - ERC-20 token configuration for amount calculations
    ///
    /// # Returns
    /// * `Ok(true)` - Address has sufficient token balance
    /// * `Ok(false)` - Address has insufficient token balance
    /// * `Err(String)` - Error occurred during balance check
    async fn check_erc20_from_address_balance(
        &self,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        token_config: &Erc20TokenConfig,
    ) -> Result<bool, String> {
        let balance =
            self.get_erc20_balance(provider, &token_config.address, from_address)
                .await
                .map_err(|e| format!("Failed to get from_address ERC-20 token balance: {}", e))?;

        info!(
            "From address {} has ERC-20 token balance: {} for token {}",
            from_address,
            format_token_amount(&balance),
            token_config.address
        );

        // For ERC-20 tokens, we don't need gas estimation as gas is paid in native tokens
        // We just need to ensure sufficient token balance for the top-up amount
        let min_required_balance = token_config.top_up_amount;

        info!(
            "From address {} requires {} tokens for token {}",
            from_address,
            format_token_amount(&min_required_balance),
            token_config.address
        );

        if balance < min_required_balance {
            warn!(
                "From address {} token balance ({}) is insufficient for top-up transaction. Required: {} for token {}",
                from_address,
                format_token_amount(&balance),
                format_token_amount(&min_required_balance),
                token_config.address
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Sends an ERC-20 token top-up transaction from one address to another.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID for wallet index lookup
    /// * `provider` - EVM provider to send the transaction through
    /// * `from_address` - Source address for the tokens
    /// * `target_address` - Destination address to receive tokens
    /// * `token_config` - ERC-20 token configuration containing amount and settings
    /// * `config` - Automatic top-up configuration containing safe address if applicable
    ///
    /// # Returns
    /// * `Ok(String)` - Transaction hash if successful
    /// * `Err(String)` - Error message if transaction fails
    async fn send_erc20_top_up_transaction(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        target_address: &EvmAddress,
        token_config: &Erc20TokenConfig,
        config: &AutomaticTopUpConfig,
    ) -> Result<String, String> {
        if from_address == target_address {
            return Err(format!(
                "Cannot send ERC-20 top-up transaction to self: from_address {} equals target_address {}",
                from_address, target_address
            ));
        }

        info!(
            "Sending ERC-20 top-up transaction: {} -> {} ({} tokens of {}){}",
            from_address,
            target_address,
            format_token_amount(&token_config.top_up_amount),
            token_config.address,
            if config.safe.is_some() { " via Safe proxy" } else { "" }
        );

        let transfer_call = IERC20::transferCall {
            to: (*target_address).into(),
            amount: token_config.top_up_amount,
        };

        // Check if we need to use Safe proxy
        let (final_to, final_value, final_data) = if let Some(safe_address) = &config.safe {
            // Use Safe proxy for ERC-20 transfer
            if let Some(ref safe_manager) = self.safe_proxy_manager {
                info!(
                    "Using Safe proxy {} for ERC-20 top-up transaction from {} to {}",
                    safe_address, from_address, target_address
                );

                // Get wallet index for signing
                let wallet_index = match self.find_wallet_index_for_address(chain_id, from_address)
                {
                    Some(index) => index,
                    None => {
                        return Err(format!(
                            "Cannot find wallet index for from_address {} on chain {}",
                            from_address, chain_id
                        ));
                    }
                };

                // Use SafeProxyManager to create the complete Safe transaction with signature
                let (_safe_tx, encoded_data) = safe_manager
                    .create_safe_transaction_with_signature(
                        provider,
                        wallet_index,
                        safe_address,
                        token_config.address, // The to address is the token contract
                        U256::ZERO,           // No ETH value for ERC-20 transfers
                        transfer_call.abi_encode().into(), // The transfer call data
                    )
                    .await
                    .map_err(|e| format!("Failed to create Safe transaction: {}", e))?;

                (*safe_address, U256::ZERO, encoded_data)
            } else {
                return Err("Safe proxy address configured but SafeProxyManager not initialized"
                    .to_string());
            }
        } else {
            // Direct ERC-20 transfer
            (token_config.address.into(), U256::ZERO, transfer_call.abi_encode().into())
        };

        let tx = TypedTransaction::Legacy(TxLegacy {
            chain_id: Some(provider.chain_id.u64()),
            nonce: 0,                  // This will be updated by the provider
            gas_price: 20_000_000_000, // 20 gwei, will be updated by gas estimation
            gas_limit: if config.safe.is_some() { 400000 } else { 100000 }, // Higher gas limit for Safe transactions
            to: alloy::primitives::TxKind::Call(final_to.into()),
            value: final_value,
            input: final_data,
        });

        // Find the wallet index for the from_address
        let wallet_index = if config.safe.is_none() {
            // For direct ERC-20 transactions
            match self.find_wallet_index_for_address(chain_id, from_address) {
                Some(index) => index,
                None => {
                    return Err(format!(
                        "Cannot find wallet index for from_address {} on chain {}",
                        from_address, chain_id
                    ));
                }
            }
        } else {
            // For Safe ERC-20 transactions, we use the signer's wallet (from_address)
            match self.find_wallet_index_for_address(chain_id, from_address) {
                Some(index) => index,
                None => {
                    return Err(format!(
                        "Cannot find wallet index for Safe signer {} on chain {}",
                        from_address, chain_id
                    ));
                }
            }
        };

        match provider.send_transaction(&wallet_index, tx).await {
            Ok(tx_hash) => {
                info!("ERC-20 top-up transaction sent successfully: {}", tx_hash);
                Ok(tx_hash.to_string())
            }
            Err(e) => {
                warn!(
                    "Failed to send ERC-20 top-up transaction from {} to {}: {}. This is non-fatal, will retry next cycle.",
                    from_address, target_address, e
                );
                Err(format!("ERC-20 transaction failed: {}", e))
            }
        }
    }

    /// Gets the ERC-20 token balance for a specific address.
    ///
    /// # Arguments
    /// * `provider` - EVM provider to query the balance
    /// * `token_address` - The ERC-20 token contract address
    /// * `wallet_address` - The wallet address to check balance for
    ///
    /// # Returns
    /// * `Ok(U256)` - Token balance if successful
    /// * `Err(String)` - Error message if query fails
    async fn get_erc20_balance(
        &self,
        provider: &EvmProvider,
        token_address: &EvmAddress,
        wallet_address: &EvmAddress,
    ) -> Result<U256, String> {
        let call_data = IERC20::balanceOfCall { account: (*wallet_address).into() };

        let call_tx = WithOtherFields::new(alloy::rpc::types::TransactionRequest {
            to: Some(alloy::primitives::TxKind::Call((*token_address).into())),
            input: Some(call_data.abi_encode().into()).into(),
            ..Default::default()
        });

        match provider.rpc_client().call(&call_tx).await {
            Ok(result) => match IERC20::balanceOfCall::abi_decode_returns(&result, false) {
                Ok(balance) => Ok(balance._0),
                Err(e) => Err(format!("Failed to decode balanceOf response: {}", e)),
            },
            Err(e) => Err(format!("Failed to call balanceOf on token contract: {}", e)),
        }
    }
}

/// Runs the automatic top-up task as a background service.
///
/// This function creates and starts an AutomaticTopUpTask instance that will
/// continuously monitor and top-up addresses based on the provided configuration.
///
/// # Arguments
/// * `config` - Setup configuration containing network and top-up settings
/// * `postgres_client` - Database client for querying relayer information
/// * `providers` - Collection of EVM providers for different blockchain networks
///
/// # Behavior
/// The task will run indefinitely, performing these operations on configured intervals:
/// - Refresh relayer cache every 60 seconds
/// - Check addresses and perform top-ups every 30 seconds
///
/// The task will only process networks that have `automatic_top_up` configured
/// in their network settings.
pub async fn run_automatic_top_up_task(
    config: SetupConfig,
    postgres_client: Arc<PostgresClient>,
    providers: Arc<Vec<EvmProvider>>,
) {
    info!("Starting automatic top-up task");

    let mut top_up_task = AutomaticTopUpTask::new(postgres_client, providers, config);

    tokio::spawn(async move {
        top_up_task.run().await;
    });
}
