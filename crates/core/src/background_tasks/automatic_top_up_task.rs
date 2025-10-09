use crate::shared::utils::{format_token_amount, format_wei_to_eth};
use crate::transaction::queue_system::TransactionToSend;

// Helper constant for gwei to wei conversion
const GWEI_TO_WEI: u128 = 1_000_000_000;
use crate::transaction::types::{TransactionData, TransactionSpeed, TransactionValue};
use crate::{
    network::ChainId,
    postgres::{PostgresClient, PostgresError},
    provider::EvmProvider,
    relayer::Relayer,
    safe_proxy::SafeProxyManager,
    shared::common_types::EvmAddress,
    shutdown::subscribe_to_shutdown,
    transaction::queue_system::TransactionsQueues,
    yaml::{AllOrAddresses, Erc20TokenConfig, NativeTokenConfig, NetworkAutomaticTopUpConfig},
    SetupConfig,
};
use alloy::primitives::U256;
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

pub struct AutomaticTopUpTask {
    postgres_client: Arc<PostgresClient>,
    providers: Arc<Vec<EvmProvider>>,
    config: SetupConfig,
    safe_proxy_manager: Arc<SafeProxyManager>,
    relayer_cache: HashMap<ChainId, Vec<Relayer>>,
    relayer_refresh_interval: Interval,
    top_up_check_interval: Interval,
    transactions_queues: Arc<tokio::sync::Mutex<TransactionsQueues>>,
}

impl AutomaticTopUpTask {
    pub async fn new(
        postgres_client: Arc<PostgresClient>,
        providers: Arc<Vec<EvmProvider>>,
        config: SetupConfig,
        transactions_queues: Arc<tokio::sync::Mutex<TransactionsQueues>>,
        safe_proxy_manager: Arc<SafeProxyManager>,
    ) -> Self {
        Self {
            postgres_client,
            providers,
            config,
            safe_proxy_manager,
            relayer_cache: HashMap::new(),
            relayer_refresh_interval: interval(Duration::from_secs(30)),
            top_up_check_interval: interval(Duration::from_secs(30)),
            transactions_queues,
        }
    }

    pub async fn run(&mut self) {
        info!("Starting automatic top-up background task");

        self.refresh_relayer_cache().await;
        let mut shutdown_rx = subscribe_to_shutdown();

        loop {
            tokio::select! {
                _ = self.relayer_refresh_interval.tick() => {
                    self.refresh_relayer_cache().await;
                }
                _ = self.top_up_check_interval.tick() => {
                    self.check_and_top_up_addresses().await;
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping automatic top-up task");
                    break;
                }
            }
        }
    }

    /// Refreshes the internal cache of relayers for all configured networks.
    async fn refresh_relayer_cache(&mut self) {
        for network_config in &self.config.networks {
            if let Some(automatic_top_up_configs) = &network_config.automatic_top_up {
                if !automatic_top_up_configs.is_empty() {
                    info!("Refreshing relayer cache for {}", network_config.name);

                    match self.get_all_relayers_for_chain(&network_config.chain_id).await {
                        Ok(relayers) => {
                            self.relayer_cache.insert(network_config.chain_id, relayers);
                        }
                        Err(e) => {
                            error!(
                                "Failed to refresh relayer cache for chain {}: {}",
                                network_config.chain_id, e
                            );
                        }
                    }
                }
            }
        }
    }

    /// Retrieves all relayers for a specific chain from the database.
    async fn get_all_relayers_for_chain(
        &self,
        chain_id: &ChainId,
    ) -> Result<Vec<Relayer>, PostgresError> {
        let all_relayers = self.postgres_client.get_all_relayers_for_chain(chain_id).await?;

        Ok(all_relayers)
    }

    /// Checks all configured addresses and performs top-ups where needed.
    async fn check_and_top_up_addresses(&self) {
        for network_config in &self.config.networks {
            if let Some(automatic_top_up_configs) = &network_config.automatic_top_up {
                if automatic_top_up_configs.is_empty() {
                    continue;
                }

                info!(
                    "Checking addresses for top-up on {} with {} configurations",
                    network_config.name,
                    automatic_top_up_configs.len()
                );

                let provider = match self.get_provider_for_chain(&network_config.chain_id) {
                    Some(p) => p,
                    None => {
                        warn!(
                            "No provider found for chain {}. Skipping top-up checks.",
                            network_config.chain_id
                        );
                        continue;
                    }
                };

                for (index, automatic_top_up) in automatic_top_up_configs.iter().enumerate() {
                    info!(
                        "Processing top-up configuration {}/{} for network {}",
                        index + 1,
                        automatic_top_up_configs.len(),
                        network_config.name
                    );
                    self.process_top_up_config(
                        &network_config.chain_id,
                        provider,
                        automatic_top_up,
                    )
                    .await;
                }
            }
        }
    }

    /// Processes a single automatic top-up configuration for a specific chain.
    async fn process_top_up_config(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        config: &NetworkAutomaticTopUpConfig,
    ) {
        info!(
            "Processing top-up config for chain {} from {}",
            chain_id, config.from.relayer.address
        );

        let relayer_addresses = match self
            .resolve_relayer_addresses(chain_id, &config.relayers, &config.from.relayer.address)
            .await
        {
            Ok(addresses) => addresses,
            Err(e) => {
                error!("Failed to resolve relayer addresses for chain {}: {}", chain_id, e);
                return;
            }
        };

        if relayer_addresses.is_empty() {
            info!("No relayer addresses found for top-up on chain {}", chain_id);
            return;
        }

        if let Some(native_config) = &config.native {
            info!("Processing native token top-ups for {} addresses", relayer_addresses.len());
            self.process_native_token_top_ups(
                chain_id,
                provider,
                &config.from.relayer.address,
                &relayer_addresses,
                native_config,
                config,
            )
            .await;
        }

        if let Some(erc20_tokens) = &config.erc20_tokens {
            for (index, token_config) in erc20_tokens.iter().enumerate() {
                info!(
                    "Processing ERC-20 token top-ups for token {} ({}/{}) on {} addresses",
                    token_config.address,
                    index + 1,
                    erc20_tokens.len(),
                    relayer_addresses.len()
                );
                self.process_erc20_token_top_ups(
                    chain_id,
                    provider,
                    &config.from.relayer.address,
                    &relayer_addresses,
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

    /// Processes native token top-ups for relayer addresses.
    async fn process_native_token_top_ups(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        relayer_addresses: &[EvmAddress],
        native_config: &NativeTokenConfig,
        config: &NetworkAutomaticTopUpConfig,
    ) {
        let mut addresses_needing_top_up = Vec::new();

        for address in relayer_addresses {
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
                relayer_addresses.len(),
                chain_id
            );
            return;
        }

        info!(
            "{} out of {} addresses need native token top-up on chain {}",
            addresses_needing_top_up.len(),
            relayer_addresses.len(),
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

    /// Processes ERC-20 token top-ups for relayer addresses.
    async fn process_erc20_token_top_ups(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        relayer_addresses: &[EvmAddress],
        token_config: &Erc20TokenConfig,
        config: &NetworkAutomaticTopUpConfig,
    ) {
        let mut addresses_needing_top_up = Vec::new();

        for address in relayer_addresses {
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
                relayer_addresses.len(),
                token_config.address,
                chain_id
            );
            return;
        }

        info!(
            "{} out of {} addresses need ERC-20 top-up for token {} on chain {}",
            addresses_needing_top_up.len(),
            relayer_addresses.len(),
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

    /// Sends a native token top-up transaction from one relayer to another.
    async fn send_native_top_up_transaction(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        relayer_address: &EvmAddress,
        native_config: &NativeTokenConfig,
        config: &NetworkAutomaticTopUpConfig,
    ) -> Result<String, String> {
        if from_address == relayer_address {
            return Err(format!(
                "Cannot send top-up transaction to self: from_address {} equals relayer_address {}",
                from_address, relayer_address
            ));
        }

        match self.postgres_client.get_relayer_by_address(relayer_address, chain_id).await {
            Ok(Some(_relayer)) => {
                // Valid relayer, proceed
            }
            Ok(None) => {
                return Err(format!(
                    "Security check failed: relayer_address {} is not a registered relayer on chain {}",
                    relayer_address, chain_id
                ));
            }
            Err(e) => {
                return Err(format!(
                    "Failed to validate relayer_address {} as relayer: {}",
                    relayer_address, e
                ));
            }
        }

        info!(
            "Sending top-up transaction: {} -> {} ({} ETH){}",
            from_address,
            relayer_address,
            format_wei_to_eth(&native_config.top_up_amount),
            if config.via_safe() { " via Safe" } else { "" }
        );

        let (final_to, final_value, final_data) = if let Some(safe_address) = &config.from.safe {
            info!(
                "Using Safe proxy {} for top-up transaction from {} to {}",
                safe_address, from_address, relayer_address
            );

            let wallet_index =
                match self.find_wallet_index_for_address(chain_id, from_address).await {
                    Some(index) => index,
                    None => {
                        return Err(format!(
                            "Cannot find wallet index for from_address {} on chain {}",
                            from_address, chain_id
                        ));
                    }
                };

            let (_safe_tx, encoded_data) = self
                .safe_proxy_manager
                .create_safe_transaction_with_signature(
                    provider,
                    wallet_index,
                    safe_address,
                    *relayer_address,
                    native_config.top_up_amount,
                    alloy::primitives::Bytes::new(),
                )
                .await
                .map_err(|e| format!("Failed to create Safe transaction: {}", e))?;

            (*safe_address, U256::ZERO, encoded_data)
        } else {
            (*relayer_address, native_config.top_up_amount, alloy::primitives::Bytes::new())
        };

        let transaction_to_send = TransactionToSend::new(
            final_to,
            TransactionValue::new(final_value),
            TransactionData::new(final_data),
            Some(TransactionSpeed::FAST),
            None,
            Some(format!("automatic_top_up_native_{}_{}", from_address, relayer_address)),
        );

        let relayer_id = if let Some(relayer) =
            self.relayer_cache.values().flatten().find(|relayer| &relayer.address == from_address)
        {
            relayer.id
        } else {
            match self.postgres_client.get_relayer_by_address(from_address, chain_id).await {
                Ok(Some(relayer)) => relayer.id,
                Ok(None) => {
                    return Err(format!(
                        "Relayer with address {} not found in database",
                        from_address
                    ))
                }
                Err(e) => {
                    return Err(format!(
                        "Database error while looking up relayer {}: {}",
                        from_address, e
                    ))
                }
            }
        };

        let mut transactions_queues = self.transactions_queues.lock().await;
        match transactions_queues.add_transaction(&relayer_id, &transaction_to_send).await {
            Ok(transaction) => {
                info!(
                    "Top-up transaction queued successfully: {} (queue transaction ID: {})",
                    transaction
                        .known_transaction_hash
                        .as_ref()
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| "pending".to_string()),
                    transaction.id
                );
                Ok(transaction.id.to_string())
            }
            Err(e) => {
                warn!(
                    "Failed to queue top-up transaction from {} to {}: {}. This is non-fatal, will retry next cycle.",
                    from_address, relayer_address, e
                );
                Err(format!("Failed to queue transaction: {}", e))
            }
        }
    }

    /// Resolves relayer addresses based on the configured relayers type.
    async fn resolve_relayer_addresses(
        &self,
        chain_id: &ChainId,
        relayers: &AllOrAddresses,
        from_address: &EvmAddress,
    ) -> Result<Vec<EvmAddress>, PostgresError> {
        let mut addresses = match relayers {
            AllOrAddresses::All => {
                match self.postgres_client.get_all_relayers_for_chain(chain_id).await {
                    Ok(relayers) => {
                        let addresses: Vec<EvmAddress> =
                            relayers.iter().map(|r| r.address).collect();
                        addresses
                    }
                    Err(e) => {
                        error!(
                            "Error fetching all the relayers on chainId {} - error {}",
                            chain_id, e
                        );
                        Vec::new()
                    }
                }
            }
            AllOrAddresses::List(addresses) => {
                // Get all relayers for this chain to validate against
                let chain_relayers =
                    match self.postgres_client.get_all_relayers_for_chain(chain_id).await {
                        Ok(relayers) => relayers.iter().map(|r| r.address).collect::<Vec<_>>(),
                        Err(e) => {
                            error!(
                                "Failed to fetch relayers for validation on chain {}: {}",
                                chain_id, e
                            );
                            return Err(e);
                        }
                    };

                // Filter addresses to only include valid relayers
                let mut valid_addresses = Vec::new();
                let mut invalid_addresses = Vec::new();

                for addr in addresses {
                    if chain_relayers.contains(addr) {
                        valid_addresses.push(*addr);
                    } else {
                        invalid_addresses.push(*addr);
                    }
                }

                if !invalid_addresses.is_empty() {
                    warn!(
                        "Ignoring {} invalid addresses on chain {} (not relayers): {:?}",
                        invalid_addresses.len(),
                        chain_id,
                        invalid_addresses
                    );
                }

                valid_addresses
            }
        };

        let contains_from_address = addresses.contains(from_address);
        // Filter out the from_address to prevent self-funding
        addresses.retain(|addr| addr != from_address);

        if contains_from_address {
            match relayers {
                AllOrAddresses::All => {
                    info!(
                        "Filtered out from_address {} from relayer targets on chain {} to prevent self-funding", 
                        from_address, chain_id
                    );
                }
                AllOrAddresses::List(_) => {
                    info!(
                        "Filtered out from_address {} from explicitly configured targets on chain {} to prevent self-funding. \
                        Note: from_address should not be included in the relayer list in YAML configuration.", 
                        from_address, chain_id
                    );
                }
            }
        }

        Ok(addresses)
    }

    /// Checks if the from_address has sufficient native balance for top-up operations.
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
            U256::from(gas_price) / U256::from(GWEI_TO_WEI),
            gas_limit
        );

        Ok(total_cost)
    }

    /// Finds the EVM provider for a specific chain ID.
    fn get_provider_for_chain(&self, chain_id: &ChainId) -> Option<&EvmProvider> {
        self.providers.iter().find(|p| &p.chain_id == chain_id)
    }

    /// Finds the wallet index for a specific address on a given chain.
    async fn find_wallet_index_for_address(
        &self,
        chain_id: &ChainId,
        address: &EvmAddress,
    ) -> Option<u32> {
        if let Some(relayers) = self.relayer_cache.get(chain_id) {
            if let Some(relayer) = relayers.iter().find(|relayer| &relayer.address == address) {
                return Some(relayer.wallet_index_type().index());
            }
        }

        info!("Wallet index for address {} not found in cache, querying database", address);

        match self.postgres_client.get_relayer_by_address(address, chain_id).await {
            Ok(Some(relayer)) => {
                info!(
                    "Found wallet index {} for address {} in database",
                    relayer.wallet_index, address
                );
                Some(relayer.wallet_index_type().index())
            }
            Ok(None) => {
                warn!(
                    "Relayer with address {} not found in database for chain {}",
                    address, chain_id
                );
                None
            }
            Err(e) => {
                error!(
                    "Database error while looking up wallet index for address {}: {}",
                    address, e
                );
                None
            }
        }
    }

    /// Checks if the from_address has sufficient ERC-20 token balance for top-up operations.
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

    /// Sends an ERC-20 token top-up transaction from one relayer to another.
    async fn send_erc20_top_up_transaction(
        &self,
        chain_id: &ChainId,
        provider: &EvmProvider,
        from_address: &EvmAddress,
        relayer_address: &EvmAddress,
        token_config: &Erc20TokenConfig,
        config: &NetworkAutomaticTopUpConfig,
    ) -> Result<String, String> {
        if from_address == relayer_address {
            return Err(format!(
                "Cannot send ERC-20 top-up transaction to self: from_address {} equals relayer_address {}",
                from_address, relayer_address
            ));
        }

        // Validate that relayer_address is a relayer for security
        match self.postgres_client.get_relayer_by_address(relayer_address, chain_id).await {
            Ok(Some(_relayer)) => {
                // Valid relayer, proceed
            }
            Ok(None) => {
                return Err(format!(
                    "Security check failed: relayer_address {} is not a registered relayer on chain {}",
                    relayer_address, chain_id
                ));
            }
            Err(e) => {
                return Err(format!(
                    "Failed to validate relayer_address {} as relayer: {}",
                    relayer_address, e
                ));
            }
        }

        info!(
            "Sending ERC-20 top-up transaction: {} -> {} ({} tokens of {}){}",
            from_address,
            relayer_address,
            format_token_amount(&token_config.top_up_amount),
            token_config.address,
            if config.via_safe() { " via Safe" } else { "" }
        );

        let transfer_call = IERC20::transferCall {
            to: (*relayer_address).into(),
            amount: token_config.top_up_amount,
        };

        let (final_to, final_value, final_data) = if let Some(safe_address) = &config.from.safe {
            info!(
                "Using Safe proxy {} for ERC-20 top-up transaction from {} to {}",
                safe_address, from_address, relayer_address
            );

            let wallet_index =
                match self.find_wallet_index_for_address(chain_id, from_address).await {
                    Some(index) => index,
                    None => {
                        return Err(format!(
                            "Cannot find wallet index for from_address {} on chain {}",
                            from_address, chain_id
                        ));
                    }
                };

            let (_safe_tx, encoded_data) = self
                .safe_proxy_manager
                .create_safe_transaction_with_signature(
                    provider,
                    wallet_index,
                    safe_address,
                    token_config.address,
                    U256::ZERO,
                    transfer_call.abi_encode().into(),
                )
                .await
                .map_err(|e| format!("Failed to create Safe transaction: {}", e))?;

            (*safe_address, U256::ZERO, encoded_data)
        } else {
            (token_config.address, U256::ZERO, transfer_call.abi_encode().into())
        };

        let transaction_to_send = TransactionToSend::new(
            final_to,
            TransactionValue::new(final_value),
            TransactionData::new(final_data),
            Some(TransactionSpeed::FAST),
            None,
            Some(format!(
                "automatic_top_up_erc20_{}_{}_{}",
                token_config.address, from_address, relayer_address
            )),
        );

        let relayer_id = if let Some(relayer) =
            self.relayer_cache.values().flatten().find(|relayer| &relayer.address == from_address)
        {
            relayer.id
        } else {
            match self.postgres_client.get_relayer_by_address(from_address, chain_id).await {
                Ok(Some(relayer)) => relayer.id,
                Ok(None) => {
                    return Err(format!(
                        "Relayer with address {} not found in database",
                        from_address
                    ))
                }
                Err(e) => {
                    return Err(format!(
                        "Database error while looking up relayer {}: {}",
                        from_address, e
                    ))
                }
            }
        };

        let mut transactions_queues = self.transactions_queues.lock().await;
        match transactions_queues.add_transaction(&relayer_id, &transaction_to_send).await {
            Ok(transaction) => {
                info!(
                    "ERC-20 top-up transaction queued successfully: {} (queue transaction ID: {})",
                    transaction
                        .known_transaction_hash
                        .as_ref()
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| "pending".to_string()),
                    transaction.id
                );
                Ok(transaction.id.to_string())
            }
            Err(e) => {
                warn!(
                    "Failed to queue ERC-20 top-up transaction from {} to {}: {}. This is non-fatal, will retry next cycle.",
                    from_address, relayer_address, e
                );
                Err(format!("Failed to queue ERC-20 transaction: {}", e))
            }
        }
    }

    /// Gets the ERC-20 token balance for a specific address.
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

        match provider.rpc_client().call(call_tx).await {
            Ok(result) => {
                if result.is_empty() {
                    return Ok(U256::ZERO);
                }

                match IERC20::balanceOfCall::abi_decode_returns(&result) {
                    Ok(balance) => Ok(balance),
                    Err(e) => {
                        warn!(
                            "Failed to decode balanceOf response for token {} and address {}: {}. \
                            Raw response length: {} bytes. Returning zero balance.",
                            token_address,
                            wallet_address,
                            e,
                            result.len()
                        );
                        Ok(U256::ZERO)
                    }
                }
            }
            Err(e) => Err(format!(
                "Failed to call balanceOf on token contract {} for address {}: {}",
                token_address, wallet_address, e
            )),
        }
    }
}

/// Runs the automatic top-up task as a background service.
///
/// This function creates and starts an AutomaticTopUpTask instance that will
/// continuously monitor and top-up addresses based on the provided configuration.
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
    transactions_queues: Arc<tokio::sync::Mutex<TransactionsQueues>>,
    safe_proxy_manager: Arc<SafeProxyManager>,
) {
    info!("Starting automatic top-up background task");

    let mut top_up_task = AutomaticTopUpTask::new(
        postgres_client,
        providers,
        config,
        transactions_queues,
        safe_proxy_manager,
    )
    .await;

    tokio::spawn(async move {
        top_up_task.run().await;
    });

    info!("Started automatic top-up background task");
}
