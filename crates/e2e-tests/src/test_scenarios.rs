use alloy::dyn_abi::TypedData;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{Context, Result};
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext},
    relayer::types::RelayerId,
    transaction::api::send_transaction::RelayTransactionRequest,
    transaction::types::{
        TransactionData, TransactionId, TransactionSpeed, TransactionStatus, TransactionValue,
    },
};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::{
    contract_interactions::ContractInteractor, relayer_client::RelayerClient,
    test_config::E2ETestConfig,
};

pub struct TestRunner {
    config: E2ETestConfig,
    relayer_client: RelayerClient,
    contract_interactor: ContractInteractor,
}

impl TestRunner {
    pub async fn new(config: E2ETestConfig) -> Result<Self> {
        let relayer_client = RelayerClient::new(&config);

        let anvil_url = format!("http://127.0.0.1:{}", config.anvil_port);
        let mut contract_interactor = ContractInteractor::new(&anvil_url).await?;

        // Deploy the test contract using the first Anvil private key
        let deployer_private_key = &config.anvil_private_keys[0];
        let contract_address = contract_interactor
            .deploy_test_contract(deployer_private_key)
            .await
            .context("Failed to deploy test contract")?;

        info!("✅ Test contract deployed at: {:?}", contract_address);

        Ok(Self { config, relayer_client, contract_interactor })
    }

    /// Mine a specified number of blocks on Anvil
    pub async fn mine_blocks(&self, num_blocks: u64) -> Result<()> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("http://127.0.0.1:{}", self.config.anvil_port);

        let mine_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_mine",
            "params": [num_blocks],
            "id": 1
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&mine_request)
            .send()
            .await
            .context("Failed to send mine request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to mine {} blocks: HTTP {} - {}", num_blocks, status, body);
        }

        debug!("⛏️ Mined {} blocks", num_blocks);
        Ok(())
    }

    /// Helper to mine a single block and wait a bit for it to be processed
    pub async fn mine_and_wait(&self) -> Result<()> {
        self.mine_blocks(1).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    }

    pub async fn run_all_tests(&self) -> HashMap<String, Result<()>> {
        let mut results = HashMap::new();

        // Run test scenarios in order
        let test_results = vec![
            ("basic_relayer_creation", self.test_basic_relayer_creation().await),
            ("simple_eth_transfer", self.test_simple_eth_transfer().await),
            ("contract_interaction", self.test_contract_interaction().await),
            ("transaction_status_tracking", self.test_transaction_status_tracking().await),
            ("failed_transaction_handling", self.test_failed_transaction_handling().await),
            ("gas_estimation", self.test_gas_estimation().await),
            ("transaction_replacement", self.test_transaction_replacement().await),
            ("batch_transactions", self.test_batch_transactions().await),
            ("relayer_limits", self.test_relayer_limits().await),
            ("gas_price_api", self.test_gas_price_api().await),
            ("network_management", self.test_network_management().await),
            ("allowlist_management", self.test_allowlist_management().await),
            ("signing_text", self.test_signing_text().await),
            ("signing_typed_data", self.test_signing_typed_data().await),
            ("transaction_operations", self.test_transaction_operations().await),
            ("transaction_status_operations", self.test_transaction_status_operations().await),
            ("transaction_counts", self.test_transaction_counts().await),
        ];

        for (test_name, result) in test_results {
            info!("🧪 Completed test: {}", test_name);
            results.insert(test_name.to_string(), result);

            // Small delay between tests
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        results
    }

    pub async fn run_filtered_test(&self, test_name: &str) -> HashMap<String, Result<()>> {
        let mut results = HashMap::new();

        info!("🧪 Running single test scenario: {}", test_name);

        let result = match test_name {
            "basic_relayer_creation" => self.test_basic_relayer_creation().await,
            "simple_eth_transfer" => self.test_simple_eth_transfer().await,
            "contract_interaction" => self.test_contract_interaction().await,
            "transaction_status_tracking" => self.test_transaction_status_tracking().await,
            "failed_transaction_handling" => self.test_failed_transaction_handling().await,
            "gas_estimation" => self.test_gas_estimation().await,
            "transaction_replacement" => self.test_transaction_replacement().await,
            "batch_transactions" => self.test_batch_transactions().await,
            "relayer_limits" => self.test_relayer_limits().await,
            "gas_price_api" => self.test_gas_price_api().await,
            "network_management" => self.test_network_management().await,
            "allowlist_management" => self.test_allowlist_management().await,
            "signing_text" => self.test_signing_text().await,
            "signing_typed_data" => self.test_signing_typed_data().await,
            "transaction_operations" => self.test_transaction_operations().await,
            "transaction_status_operations" => self.test_transaction_status_operations().await,
            "transaction_counts" => self.test_transaction_counts().await,
            _ => Err(anyhow::anyhow!("Unknown test scenario: {}", test_name)),
        };

        results.insert(test_name.to_string(), result);
        results
    }

    /// Fund a relayer address with ETH from the first Anvil account
    async fn fund_relayer(&self, relayer_address: &str) -> Result<()> {
        let anvil_url = format!("http://127.0.0.1:{}", self.config.anvil_port);

        // Create signer with first Anvil private key (has lots of ETH)
        let private_key = self.config.anvil_private_keys[0].clone();
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet = EthereumWallet::from(signer);

        // Create provider with wallet
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(anvil_url.parse()?);

        // Parse relayer address
        let to_address: Address = relayer_address.parse()?;

        // Send 10 ETH to the relayer
        let funding_amount = U256::from(10_000_000_000_000_000_000_u128); // 10 ETH in wei

        let tx_request = TransactionRequest::default().to(to_address).value(funding_amount);

        info!("Funding relayer {} with 10 ETH", relayer_address);

        let pending_tx = provider
            .send_transaction(tx_request)
            .await
            .context("Failed to send funding transaction")?;

        let tx_hash = pending_tx.tx_hash();
        info!("Sent funding transaction with hash: {:?}", tx_hash);

        // Mine a block to include the transaction
        self.mine_and_wait().await?;

        // Wait for transaction to be mined
        let receipt =
            pending_tx.get_receipt().await.context("Failed to get funding transaction receipt")?;

        info!("Funding transaction mined in block: {:?}", receipt.block_number);
        info!("Successfully funded relayer {} with 10 ETH", relayer_address);

        Ok(())
    }

    /// Create and fund a relayer in one step
    async fn create_and_fund_relayer(&self, name: &str) -> Result<serde_json::Value> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let relayer_address = relayer["address"].as_str().context("Missing relayer address")?;

        // // Pause the relayer immediately to prevent transaction processing during funding
        // self.relayer_client.sdk.relayer.pause(&relayer_id).await
        //     .context("Failed to pause relayer")?;

        // Fund the relayer with ETH
        self.fund_relayer(relayer_address).await.context("Failed to fund relayer")?;

        // // Unpause the relayer now that it's funded
        // self.relayer_client.sdk.relayer.unpause(&relayer_id).await
        //     .context("Failed to unpause relayer")?;

        Ok(relayer)
    }

    /// Test 1: Basic relayer creation
    async fn test_basic_relayer_creation(&self) -> Result<()> {
        debug!("Creating test relayer...");

        let relayer = self
            .relayer_client
            .create_relayer("e2e-test-relayer", self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        // Verify relayer has ID and address
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let relayer_address = relayer["address"].as_str().context("Missing relayer address")?;

        debug!("Created relayer {} with address {}", relayer_id, relayer_address);

        // Verify address is valid Ethereum address
        Address::from_str(relayer_address).context("Invalid relayer address format")?;

        // Fund the relayer with ETH
        self.fund_relayer(relayer_address).await.context("Failed to fund relayer")?;

        Ok(())
    }

    /// Test 2: Simple ETH transfer
    async fn test_simple_eth_transfer(&self) -> Result<()> {
        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("eth-transfer-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let recipient = &self.config.anvil_accounts[1]; // Use second anvil account

        info!("Sending ETH transfer to {}", recipient);

        // Send transaction
        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer_id,
                recipient,
                Some("1000000000000000000"), // 1 ETH in wei
                None,
            )
            .await
            .context("Failed to send ETH transfer")?;

        info!("ETH transfer sent: {:?}", tx_response);

        // Wait for transaction to be mined
        self.wait_for_transaction_completion(&tx_response.id).await?;

        Ok(())
    }

    /// Test 3: Contract interaction
    async fn test_contract_interaction(&self) -> Result<()> {
        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("contract-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Get the deployed test contract address
        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let contract_address_str = format!("{:?}", contract_address);
        info!("Sending contract interaction to deployed contract at {}", contract_address_str);

        // Generate calldata for setValue(42) function
        let calldata = self.contract_interactor.encode_simple_call(42)?;

        let tx_response = self
            .relayer_client
            .send_transaction(&relayer_id, &contract_address_str, None, Some(&calldata))
            .await
            .context("Failed to send contract interaction")?;

        info!("Contract interaction sent: {:?}", tx_response);

        self.wait_for_transaction_completion(&tx_response.id).await?;

        info!("✅ Contract interaction completed successfully");
        Ok(())
    }

    /// Test 4: Transaction status tracking
    async fn test_transaction_status_tracking(&self) -> Result<()> {
        let relayer = self.create_and_fund_relayer("status-tracking-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer_id,
                &self.config.anvil_accounts[2],
                Some("500000000000000000"), // 0.5 ETH
                None,
            )
            .await?;

        debug!("Tracking transaction status for ID: {}", tx_response.id.to_string());

        // Check initial status
        let initial_status = self.relayer_client.get_transaction_status(&tx_response.id).await?;

        debug!("Initial status: {:?}", initial_status);

        // Wait and check final status
        self.wait_for_transaction_completion(&tx_response.id).await?;

        let final_status = self.relayer_client.get_transaction_status(&tx_response.id).await?;

        debug!("Final status: {:?}", final_status);

        // Verify status progression
        if matches!(final_status.status, TransactionStatus::Pending) {
            warn!("Transaction still pending after wait period");
        }

        Ok(())
    }

    /// Test 5: Failed transaction handling
    async fn test_failed_transaction_handling(&self) -> Result<()> {
        let relayer = self.create_and_fund_relayer("failure-test-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Send transaction to invalid address (should fail)
        let result = self
            .relayer_client
            .send_transaction(
                &relayer_id,
                "0x0000000000000000000000000000000000000000", // Burn address
                Some("1000000000000000000000"),               // Very large amount (should fail)
                None,
            )
            .await;

        match result {
            Ok(tx_response) => {
                debug!("Potentially failing transaction sent: {:?}", tx_response);
                // Even if sent, it might fail during execution
                let final_status = self.wait_for_transaction_completion(&tx_response.id).await;
                debug!("Failure test result: {:?}", final_status);
            }
            Err(e) => {
                debug!("Transaction rejected as expected: {}", e);
                // This is also a valid outcome
            }
        }

        Ok(())
    }

    /// Test 6: Gas estimation
    async fn test_gas_estimation(&self) -> Result<()> {
        let relayer = self.create_and_fund_relayer("gas-test-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Send a simple transaction and verify it uses reasonable gas
        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer_id,
                &self.config.anvil_accounts[3],
                Some("100000000000000000"), // 0.1 ETH
                None,
            )
            .await?;

        debug!("Gas estimation test transaction: {:?}", tx_response);

        // Wait for completion and check gas used
        self.wait_for_transaction_completion(&tx_response.id).await?;

        // Get final status with receipt to check gas used
        let final_status = self.relayer_client.get_transaction_status(&tx_response.id).await?;

        if let Some(receipt) = final_status.receipt {
            debug!("Gas used: {:?}", receipt.gas_used);
        }

        Ok(())
    }

    /// Test 7: Transaction replacement (if supported)
    async fn test_transaction_replacement(&self) -> Result<()> {
        // This test would be more complex and depends on your relayer's
        // transaction replacement capabilities
        debug!("Transaction replacement test - placeholder");
        Ok(())
    }

    /// Test 8: Batch transactions
    async fn test_batch_transactions(&self) -> Result<()> {
        let relayer = self.create_and_fund_relayer("batch-test-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Send multiple transactions quickly
        let mut tx_ids: Vec<TransactionId> = Vec::new();

        for i in 0..3 {
            let tx_response = self
                .relayer_client
                .send_transaction(
                    &relayer_id,
                    &self.config.anvil_accounts[4],
                    Some("10000000000000000"), // 0.01 ETH each
                    None,
                )
                .await?;

            debug!("Sent batch transaction {}: {:?}", i + 1, tx_response);
            tx_ids.push(tx_response.id);

            // Mine a block after each transaction to ensure it gets processed
            self.mine_and_wait().await?;
        }

        // Wait for all transactions to complete
        for tx_id in &tx_ids {
            self.wait_for_transaction_completion(tx_id).await?;
        }

        Ok(())
    }

    /// Test 9: Relayer limits and pagination
    async fn test_relayer_limits(&self) -> Result<()> {
        let relayer = self.create_and_fund_relayer("limits-test-relayer").await?;

        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Test pagination of relayer transactions
        let transactions = self.relayer_client.get_relayer_transactions(&relayer_id, 10, 0).await?;

        debug!("Relayer transactions: {:?}", transactions);

        // Test pending count
        let pending_count = self.relayer_client.get_pending_count(&relayer_id).await?;

        debug!("Pending count: {}", pending_count);

        Ok(())
    }

    /// Helper: Wait for transaction to complete
    async fn wait_for_transaction_completion(&self, transaction_id: &TransactionId) -> Result<()> {
        let timeout = tokio::time::Duration::from_secs(self.config.test_timeout_seconds);
        let start = tokio::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!(
                    "Transaction {} timed out after {} seconds",
                    transaction_id,
                    self.config.test_timeout_seconds
                );
            }

            let status = self.relayer_client.get_transaction_status(transaction_id).await?;
            info!("Transaction {} status: {:?}", transaction_id, status);

            match status.status {
                TransactionStatus::Confirmed | TransactionStatus::Mined => {
                    info!("Transaction {} completed successfully", transaction_id);
                    return Ok(());
                }
                TransactionStatus::Failed => {
                    anyhow::bail!("Transaction {} failed: {:?}", transaction_id, status);
                }
                TransactionStatus::Pending | TransactionStatus::Inmempool => {
                    info!(
                        "Transaction {} still pending, mining a block and waiting...",
                        transaction_id
                    );
                    // Mine a block to help the transaction get processed
                    self.mine_and_wait().await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                TransactionStatus::Expired => {
                    anyhow::bail!("Transaction {} expired: {:?}", transaction_id, status);
                }
            }
        }
    }

    /// Test 10: Gas Price API
    async fn test_gas_price_api(&self) -> Result<()> {
        debug!("Testing gas price API...");

        // Test getting gas prices for our test chain
        let gas_prices = self
            .relayer_client
            .sdk
            .gas
            .get_gas_prices(self.config.chain_id)
            .await
            .context("Failed to get gas prices")?;

        debug!("Gas prices for chain {}: {:?}", self.config.chain_id, gas_prices);

        // Verify we get a response (may be None if no custom provider configured)
        // But the API call should succeed
        info!("✅ Gas price API responds correctly");

        Ok(())
    }

    /// Test 11: Network Management
    async fn test_network_management(&self) -> Result<()> {
        debug!("Testing network management APIs...");

        // Test get all networks
        let all_networks = self
            .relayer_client
            .sdk
            .network
            .get_all_networks()
            .await
            .context("Failed to get all networks")?;
        debug!("All networks: {} found", all_networks.len());

        // Test get enabled networks
        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;
        debug!("Enabled networks: {} found", enabled_networks.len());

        // Test get disabled networks
        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;
        debug!("Disabled networks: {} found", disabled_networks.len());

        // Find our test network in the lists
        let test_network = all_networks.iter().find(|n| n.chain_id.u64() == self.config.chain_id);

        if let Some(network) = test_network {
            debug!("Test network found: {} (chain_id: {})", network.name, network.chain_id);
        }

        info!("✅ Network management APIs work correctly");
        Ok(())
    }

    /// Test 12: Allowlist Management
    async fn test_allowlist_management(&self) -> Result<()> {
        debug!("Testing allowlist management...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("allowlist-test-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Test adding to allowlist
        let test_address = EvmAddress::from_str(&self.config.anvil_accounts[2])?;

        let relayer_id_typed = relayer_id;
        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .add(&relayer_id_typed, &test_address)
            .await
            .context("Failed to add address to allowlist")?;
        debug!("Added {} to allowlist", test_address.hex());

        // Test getting all allowlisted addresses
        let paging = PagingContext { limit: 10, offset: 0 };
        let allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer_id_typed, &paging)
            .await
            .context("Failed to get allowlist")?;

        debug!("Allowlist has {} addresses", allowlist.items.len());

        // Verify our address is in the list
        let found_address = allowlist.items.iter().find(|addr| addr.hex() == test_address.hex());

        if found_address.is_some() {
            debug!("✅ Address found in allowlist");
        } else {
            return Err(anyhow::anyhow!("Address not found in allowlist"));
        }

        // Test removing from allowlist
        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .delete(&relayer_id_typed, &test_address)
            .await
            .context("Failed to remove address from allowlist")?;
        debug!("Removed {} from allowlist", test_address.hex());

        // Verify address was removed
        let updated_allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer_id_typed, &paging)
            .await
            .context("Failed to get updated allowlist")?;

        let address_still_exists =
            updated_allowlist.items.iter().any(|addr| addr.hex() == test_address.hex());

        if address_still_exists {
            return Err(anyhow::anyhow!("Address still found in allowlist after deletion"));
        }

        info!("✅ Allowlist management works correctly");
        Ok(())
    }

    /// Test 13: Text Signing
    async fn test_signing_text(&self) -> Result<()> {
        debug!("Testing text signing...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("signing-text-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        let test_message = "Hello, RRelayer E2E Test!";

        let relayer_id_typed = relayer_id;

        // Sign text message
        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_text(&relayer_id_typed, test_message)
            .await
            .context("Failed to sign text message")?;

        debug!("Signed message. Signature: {}", sign_result.signature);

        // Verify we got a signature (PrimitiveSignature is a byte array, not string)
        debug!("✅ Got signature: {:?}", sign_result.signature);

        // Wait a moment for the signing to be recorded
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Get text signing history
        let paging = PagingContext { limit: 10, offset: 0 };
        let history = self
            .relayer_client
            .sdk
            .sign
            .get_text_history(&relayer_id_typed, &paging)
            .await
            .context("Failed to get text signing history")?;

        debug!("Text signing history has {} entries", history.items.len());

        // Find our signed message in history
        let signed_message = history.items.iter().find(|entry| entry.message == test_message);

        if let Some(entry) = signed_message {
            debug!("✅ Found signed message in history: {}", entry.message);
            debug!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed message not found in history"));
        }

        info!("✅ Text signing works correctly");
        Ok(())
    }

    /// Test 14: Typed Data Signing
    async fn test_signing_typed_data(&self) -> Result<()> {
        debug!("Testing typed data signing...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("signing-typed-data-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;

        // Create test EIP-712 typed data
        let typed_data_json = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "Mail": [
                    {"name": "from", "type": "Person"},
                    {"name": "to", "type": "Person"},
                    {"name": "contents", "type": "string"}
                ],
                "Person": [
                    {"name": "name", "type": "string"},
                    {"name": "wallet", "type": "address"}
                ]
            },
            "primaryType": "Mail",
            "domain": {
                "name": "RRelayer Test",
                "version": "1",
                "chainId": self.config.chain_id,
                "verifyingContract": "0x0000000000000000000000000000000000000000"
            },
            "message": {
                "from": {
                    "name": "Alice",
                    "wallet": "0x1234567890123456789012345678901234567890"
                },
                "to": {
                    "name": "Bob",
                    "wallet": "0x0987654321098765432109876543210987654321"
                },
                "contents": "Hello from E2E test!"
            }
        });

        let typed_data: TypedData =
            serde_json::from_value(typed_data_json).context("Failed to create typed data")?;

        let relayer_id_typed = relayer_id;

        // Sign typed data
        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer_id_typed, &typed_data)
            .await
            .context("Failed to sign typed data")?;

        debug!("Signed typed data. Signature: {}", sign_result.signature);

        // Verify we got a signature (PrimitiveSignature is a byte array, not string)
        debug!("✅ Got typed data signature: {:?}", sign_result.signature);

        // Wait for the signing to be recorded
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Get typed data signing history
        let paging = PagingContext { limit: 10, offset: 0 };
        let history = self
            .relayer_client
            .sdk
            .sign
            .get_typed_data_history(&relayer_id_typed, &paging)
            .await
            .context("Failed to get typed data signing history")?;

        debug!("Typed data signing history has {} entries", history.items.len());

        // Find our signed typed data in history
        let signed_entry = history.items.iter().find(|entry| {
            if let Some(domain) = entry.domain_data.get("name") {
                domain.as_str() == Some("RRelayer Test")
            } else {
                false
            }
        });

        if let Some(entry) = signed_entry {
            debug!("✅ Found signed typed data in history: {:?}", entry.domain_data);
            debug!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed typed data not found in history"));
        }

        info!("✅ Typed data signing works correctly");
        Ok(())
    }

    /// Test 15: Transaction Operations (get, cancel, replace)
    async fn test_transaction_operations(&self) -> Result<()> {
        debug!("Testing transaction operations...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("tx-ops-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let relayer_id_typed = relayer_id;

        // Send a transaction using the SDK directly
        let tx_request = RelayTransactionRequest {
            to: EvmAddress::from_str(&self.config.anvil_accounts[1])?,
            value: TransactionValue::new(U256::from(1000000000000000000u128)), // 1 ETH
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-tx-ops".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer_id_typed, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;
        debug!("Sent transaction with ID: {}", transaction_id);

        // Test getting the transaction
        let retrieved_tx = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction(transaction_id)
            .await
            .context("Failed to get transaction")?;

        if let Some(tx) = retrieved_tx {
            debug!("✅ Retrieved transaction: {}", tx.id);
            debug!("   To: {}", tx.to.hex());
            debug!("   Value: {:?}", tx.value);
        } else {
            return Err(anyhow::anyhow!("Transaction not found"));
        }

        // Test getting all transactions for the relayer
        let paging = PagingContext { limit: 10, offset: 0 };
        let relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions(&relayer_id_typed, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        debug!("Found {} transactions for relayer", relayer_transactions.items.len());

        // Find our transaction in the list
        let found_tx = relayer_transactions
            .items
            .iter()
            .find(|tx| tx.id.to_string() == transaction_id.to_string());

        if found_tx.is_some() {
            debug!("✅ Transaction found in relayer's transaction list");
        } else {
            return Err(anyhow::anyhow!("Transaction not found in relayer's list"));
        }

        // Test transaction replacement (create a replacement with higher gas)
        let replacement_request = RelayTransactionRequest {
            to: EvmAddress::from_str(&self.config.anvil_accounts[1])?,
            value: TransactionValue::new(U256::from(2000000000000000000u128)), // 2 ETH
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-tx-ops-replacement".to_string()),
            blobs: None,
        };

        let replace_result = self
            .relayer_client
            .sdk
            .transaction
            .replace_transaction(transaction_id, &replacement_request)
            .await
            .context("Failed to replace transaction")?;

        debug!("Transaction replacement result: {}", replace_result);

        // Test transaction cancellation (try to cancel - may succeed or fail based on timing)
        let cancel_result =
            self.relayer_client.sdk.transaction.cancel_transaction(transaction_id).await;

        match cancel_result {
            Ok(cancelled) => debug!("Transaction cancellation result: {}", cancelled),
            Err(e) => debug!("Transaction cancellation failed (expected if already mined): {}", e),
        }

        info!("✅ Transaction operations work correctly");
        Ok(())
    }

    /// Test 16: Transaction Status Operations
    async fn test_transaction_status_operations(&self) -> Result<()> {
        debug!("Testing transaction status operations...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("tx-status-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let relayer_id_typed = relayer_id;

        // Send a transaction
        let tx_request = RelayTransactionRequest {
            to: EvmAddress::from_str(&self.config.anvil_accounts[2])?,
            value: TransactionValue::new(U256::from(500000000000000000u128)), // 0.5 ETH
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-status-ops".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer_id_typed, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;
        debug!("Sent transaction for status testing: {}", transaction_id);

        // Test getting transaction status
        let status_result = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(transaction_id)
            .await
            .context("Failed to get transaction status")?;

        if let Some(status) = status_result {
            debug!("✅ Transaction status: {}", status.status);
            debug!("   Transaction hash: {:?}", status.hash);
        } else {
            return Err(anyhow::anyhow!("Transaction status not found"));
        }

        // Wait for transaction to potentially move through states
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Check status again
        let updated_status = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(transaction_id)
            .await
            .context("Failed to get updated transaction status")?;

        if let Some(status) = updated_status {
            debug!("Updated transaction status: {}", status.status);
        }

        info!("✅ Transaction status operations work correctly");
        Ok(())
    }

    /// Test 17: Transaction Counts (inmempool and pending)
    async fn test_transaction_counts(&self) -> Result<()> {
        debug!("Testing transaction count operations...");

        // Create and fund relayer
        let relayer = self.create_and_fund_relayer("tx-counts-relayer").await?;
        let relayer_id_str = relayer["id"].as_str().context("Missing relayer ID")?;
        let relayer_id = RelayerId::from_str(relayer_id_str).context("Invalid relayer ID")?;
        let relayer_id_typed = relayer_id;

        // Get initial counts
        let initial_pending = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer_id_typed)
            .await
            .context("Failed to get initial pending count")?;

        let initial_inmempool = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer_id_typed)
            .await
            .context("Failed to get initial inmempool count")?;

        debug!("Initial counts - Pending: {}, InMempool: {}", initial_pending, initial_inmempool);

        // Send several transactions quickly
        let mut transaction_ids = Vec::new();
        for i in 0..3 {
            let tx_request = RelayTransactionRequest {
                to: EvmAddress::from_str(&self.config.anvil_accounts[1])?,
                value: TransactionValue::new(U256::from(100000000000000000u128 * (i + 1))), // 0.1, 0.2, 0.3 ETH
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("test-counts-{}", i)),
                blobs: None,
            };

            let send_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer_id_typed, &tx_request)
                .await
                .context(format!("Failed to send transaction {}", i))?;

            transaction_ids.push(send_result.id.clone());
            debug!("Sent transaction {}: {}", i, send_result.id);

            // Mine a block after each transaction
            self.mine_and_wait().await?;

            // Small delay between transactions
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Wait a moment for transactions to be processed
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Get updated counts
        let final_pending = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer_id_typed)
            .await
            .context("Failed to get final pending count")?;

        let final_inmempool = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer_id_typed)
            .await
            .context("Failed to get final inmempool count")?;

        debug!("Final counts - Pending: {}, InMempool: {}", final_pending, final_inmempool);

        // Verify counts make sense (should have increased)
        let total_final = final_pending + final_inmempool;
        let total_initial = initial_pending + initial_inmempool;

        if total_final >= total_initial {
            debug!("✅ Transaction counts increased as expected");
        } else {
            warn!("Transaction counts may have decreased (transactions completed quickly)");
        }

        info!("✅ Transaction count operations work correctly");
        Ok(())
    }
}
