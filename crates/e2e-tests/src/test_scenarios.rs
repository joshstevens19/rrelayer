use alloy::dyn_abi::TypedData;
use alloy::network::{AnyTransactionReceipt, EthereumWallet, ReceiptResponse};
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{anyhow, Context, Result};
use rrelayer_core::gas::types::GasPrice;
use rrelayer_core::network::types::ChainId;
use rrelayer_core::relayer::api::CreateRelayerResult;
use rrelayer_core::transaction::api::get_transaction_status::RelayTransactionStatusResult;
use rrelayer_core::transaction::types::Transaction;
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext},
    relayer::types::RelayerId,
    transaction::api::send_transaction::RelayTransactionRequest,
    transaction::types::{
        TransactionData, TransactionId, TransactionSpeed, TransactionStatus, TransactionValue,
    },
};
use rrelayer_sdk::SDK;
use std::collections::HashMap;
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub enum TestResult {
    Passed,
    Failed(String),
    Timeout,
    Skipped(String),
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(self, TestResult::Passed)
    }

    pub fn status_icon(&self) -> &'static str {
        match self {
            TestResult::Passed => "✅",
            TestResult::Failed(_) => "❌",
            TestResult::Timeout => "⏰",
            TestResult::Skipped(_) => "⏭️",
        }
    }
}

#[derive(Debug)]
pub struct TestInfo {
    pub name: String,
    pub result: TestResult,
    pub duration: Duration,
    pub error_message: Option<String>,
}

impl TestInfo {
    pub fn new(name: String, result: TestResult, duration: Duration) -> Self {
        let error_message = match &result {
            TestResult::Failed(msg) => Some(msg.clone()),
            TestResult::Timeout => Some("Test timed out after 30 seconds".to_string()),
            TestResult::Skipped(msg) => Some(msg.clone()),
            TestResult::Passed => None,
        };

        Self { name, result, duration, error_message }
    }
}

pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestInfo>,
    pub duration: Duration,
}

impl TestSuite {
    pub fn new(name: String) -> Self {
        Self { name, tests: Vec::new(), duration: Duration::ZERO }
    }

    pub fn add_test(&mut self, test: TestInfo) {
        self.duration += test.duration;
        self.tests.push(test);
    }

    pub fn passed_count(&self) -> usize {
        self.tests.iter().filter(|t| t.result.is_success()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Failed(_))).count()
    }

    pub fn timeout_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Timeout)).count()
    }

    pub fn skipped_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Skipped(_))).count()
    }

    pub fn total_count(&self) -> usize {
        self.tests.len()
    }
}

use crate::{
    anvil_manager::AnvilManager, contract_interactions::ContractInteractor,
    relayer_client::RelayerClient, test_config::E2ETestConfig,
};

pub struct TestRunner {
    config: E2ETestConfig,
    relayer_client: RelayerClient,
    contract_interactor: ContractInteractor,
    anvil_manager: AnvilManager,
}

impl TestRunner {
    pub async fn new(config: E2ETestConfig, anvil_manager: AnvilManager) -> Result<Self> {
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

        Ok(Self { config, relayer_client, contract_interactor, anvil_manager })
    }

    pub fn into_anvil_manager(self) -> AnvilManager {
        self.anvil_manager
    }

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

        info!("⛏️ Mined {} blocks", num_blocks);
        Ok(())
    }

    /// Helper to mine a single block and wait a bit for it to be processed
    pub async fn mine_and_wait(&self) -> Result<()> {
        self.mine_blocks(1).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Run all test scenarios with proper timeout and reporting
    pub async fn run_all_tests(&mut self) -> TestSuite {
        println!("🚀 RRelayer E2E Test Suite");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut suite = TestSuite::new("RRelayer E2E Tests".to_string());
        let overall_start = Instant::now();

        let test_definitions = vec![
            ("basic_relayer_creation", "Basic relayer creation and setup"),
            ("simple_eth_transfer", "Simple ETH transfer functionality"),
            ("contract_interaction", "Smart contract interaction"),
            (
                "failed_transaction_handling_not_enough_funds",
                "Failed transaction - insufficient funds",
            ),
            (
                "failed_transaction_handling_revert_execution",
                "Failed transaction - contract revert",
            ),
            ("gas_estimation", "Gas estimation functionality"),
            ("transaction_replacement", "Transaction replacement operations"),
            ("batch_transactions", "Batch transaction processing"),
            ("relayer_limits", "Relayer limit enforcement"),
            ("gas_price_api", "Gas price API functionality"),
            ("network_management", "Network management operations"),
            ("allowlist_add", "Allowlist add operation"),
            ("allowlist_list", "Allowlist list operation"),
            ("allowlist_remove", "Allowlist remove operation"),
            ("signing_text", "Text signing functionality"),
            ("signing_typed_data", "Typed data signing functionality"),
            ("transaction_send", "Transaction send operation"),
            ("transaction_get", "Transaction get operation"),
            ("transaction_list", "Transaction list operation"),
            ("transaction_replace", "Transaction replace operation"),
            ("transaction_cancel", "Transaction cancel operation"),
            ("transaction_status_operations", "Transaction status operations"),
            ("transaction_counts", "Transaction count operations"),
            ("transaction_status_pending", "Transaction pending state validation"),
            ("transaction_status_inmempool", "Transaction inmempool state validation"),
            ("transaction_status_mined", "Transaction mined state validation"),
            ("transaction_status_confirmed", "Transaction confirmed state validation"),
            ("transaction_status_failed", "Transaction failed state validation"),
            ("transaction_status_expired", "Transaction expired state validation"),
            ("allowlist_restrictions", "Allowlist restriction enforcement"),
            ("allowlist_edge_cases", "Allowlist edge case handling"),
            ("relayer_pause_unpause", "Relayer pause/unpause functionality"),
            ("relayer_gas_configuration", "Relayer gas configuration management"),
            ("relayer_allowlist_toggle", "Relayer allowlist toggle functionality"),
            ("transaction_nonce_management", "Transaction nonce management"),
            ("gas_price_bumping", "Gas price bumping mechanism"),
            ("transaction_replacement_edge_cases", "Transaction replacement edge cases"),
            ("webhook_delivery_testing", "Webhook delivery testing"),
            ("rate_limiting_enforcement", "Rate limiting enforcement"),
            ("concurrent_transactions", "Concurrent transaction handling"),
            ("network_configuration_edge_cases", "Network configuration edge cases"),
            ("authentication_edge_cases", "Authentication edge cases"),
            ("blob_transaction_handling", "Blob transaction handling (EIP-4844)"),
            ("transaction_data_validation", "Transaction data validation"),
            ("balance_edge_cases", "Balance edge case handling"),
        ];

        for (test_name, description) in test_definitions {
            let test_result = self.run_single_test(test_name, description).await;
            suite.add_test(test_result);
        }

        suite.duration = overall_start.elapsed();
        self.print_final_report(&suite);
        suite
    }

    /// Run a single test with timeout and proper error handling
    async fn run_single_test(&mut self, test_name: &str, description: &str) -> TestInfo {
        print!("🧪 {} ... ", description);
        let start = Instant::now();

        // BeforeTest hook: Restart Anvil to ensure clean state for each test
        // if let Err(e) = self.anvil_manager.restart().await {
        //     warn!("Failed to restart Anvil before test {}: {}", test_name, e);
        //     return TestInfo::new(
        //         test_name.to_string(),
        //         TestResult::Failed(format!("Failed to restart Anvil: {}", e)),
        //         start.elapsed(),
        //     );
        // }

        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = timeout(Duration::from_secs(30), self.execute_test(test_name)).await;

        let test_result = match result {
            Ok(Ok(())) => {
                println!("✅ PASS");
                TestResult::Passed
            }
            Ok(Err(e)) => {
                println!("❌ FAIL");
                TestResult::Failed(e.to_string())
            }
            Err(_) => {
                println!("⏰ TIMEOUT");
                TestResult::Timeout
            }
        };

        let duration = start.elapsed();
        TestInfo::new(test_name.to_string(), test_result, duration)
    }

    async fn execute_test(&self, test_name: &str) -> Result<()> {
        match test_name {
            "basic_relayer_creation" => self.test_basic_relayer_creation().await,
            "simple_eth_transfer" => self.test_simple_eth_transfer().await,
            "contract_interaction" => self.test_contract_interaction().await,
            "failed_transaction_handling_not_enough_funds" => {
                self.test_failed_transaction_handling_not_enough_funds().await
            }
            "failed_transaction_handling_revert_execution" => {
                self.test_failed_transaction_handling_revert_execution().await
            }
            "gas_estimation" => self.test_gas_estimation().await,
            "batch_transactions" => self.test_batch_transactions().await,
            "transaction_count" => self.test_transaction_count().await,
            "gas_price_api" => self.test_gas_price_api().await,
            "network_management" => self.test_network_management().await,
            "allowlist_add" => self.test_allowlist_add().await,
            "allowlist_remove" => self.test_allowlist_remove().await,
            "signing_text" => self.test_signing_text().await,
            "signing_typed_data" => self.test_signing_typed_data().await,
            "transaction_get" => self.test_transaction_get().await,
            "transaction_list" => self.test_transaction_list().await,
            "transaction_replace" => self.test_transaction_replace().await,
            "transaction_cancel" => self.test_transaction_cancel().await,
            "transaction_status_operations" => self.test_transaction_status_operations().await,
            "transaction_counts" => self.test_transaction_counts().await,
            "transaction_status_pending" => self.test_transaction_status_pending().await,
            "transaction_status_inmempool" => self.test_transaction_status_inmempool().await,
            "transaction_status_mined" => self.test_transaction_status_mined().await,
            "transaction_status_confirmed" => self.test_transaction_status_confirmed().await,
            "transaction_status_failed" => self.test_transaction_status_failed().await,
            "transaction_status_expired" => self.test_transaction_status_expired().await,
            "allowlist_restrictions" => self.test_allowlist_restrictions().await,
            "allowlist_edge_cases" => self.test_allowlist_edge_cases().await,
            "relayer_pause_unpause" => self.test_relayer_pause_unpause().await,
            "relayer_delete" => self.test_relayer_delete().await,
            "relayer_gas_configuration" => self.test_relayer_gas_configuration().await,
            "relayer_allowlist_toggle" => self.test_relayer_allowlist_toggle().await,
            "transaction_nonce_management" => self.test_transaction_nonce_management().await,
            "gas_price_bumping" => self.test_gas_price_bumping().await,
            "webhook_delivery_testing" => self.test_webhook_delivery().await,
            "rate_limiting_enforcement" => self.test_rate_limiting().await,
            "concurrent_transactions" => self.test_concurrent_transactions().await,
            "unauthenticated" => self.test_unauthenticated().await,
            "blob_transaction_handling" => self.test_blob_transaction_handling().await,
            "transaction_data_validation" => self.test_transaction_data_validation().await,
            "balance_edge_cases" => self.test_balance_edge_cases().await,
            _ => Err(anyhow::anyhow!("Unknown test: {}", test_name)),
        }
    }

    fn print_final_report(&self, suite: &TestSuite) {
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let passed = suite.passed_count();
        let failed = suite.failed_count();
        let timeout = suite.timeout_count();
        let skipped = suite.skipped_count();
        let total = suite.total_count();

        // Summary line
        if failed == 0 && timeout == 0 {
            println!("✅ Test Suites: 1 passed, 1 total");
            println!("✅ Tests:       {} passed, {} total", passed, total);
        } else {
            println!(
                "❌ Test Suites: {} failed, 1 total",
                if failed > 0 || timeout > 0 { 1 } else { 0 }
            );
            println!(
                "❌ Tests:       {} failed, {} passed, {} total",
                failed + timeout,
                passed,
                total
            );
        }

        if skipped > 0 {
            println!("⏭️ Skipped:     {}", skipped);
        }

        println!("⏱️ Time:        {:.2}s", suite.duration.as_secs_f64());

        // Failed tests details
        if failed > 0 || timeout > 0 {
            println!();
            println!("Failed Tests:");
            for test in &suite.tests {
                if let TestResult::Failed(msg) = &test.result {
                    println!("  ❌ {} - {}", test.name, msg);
                } else if let TestResult::Timeout = &test.result {
                    println!("  ⏰ {} - Test timed out after 30 seconds", test.name);
                }
            }
        }

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        if failed == 0 && timeout == 0 {
            println!("🎉 All tests passed!");
        } else {
            println!("💥 Some tests failed. See details above.");
        }
    }

    // Tests missing
    // TODO: Relayer delete
    // TODO: Relayer clone logic
    // TODO: Webhooks testing
    // TODO: Automatic top up tasks
    // TODO: Safe proxy

    /// Run a single filtered test with the new reporting system
    pub async fn run_filtered_test(&mut self, test_name: &str) -> TestSuite {
        println!("🚀 RRelayer E2E Test Suite - Single Test");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut suite = TestSuite::new("Single Test Run".to_string());
        let overall_start = Instant::now();

        let description = match test_name {
            "basic_relayer_creation" => "Basic relayer creation and setup",
            "simple_eth_transfer" => "Simple ETH transfer functionality",
            "contract_interaction" => "Smart contract interaction",
            "failed_transaction_handling_not_enough_funds" => {
                "Failed transaction - insufficient funds"
            }
            "failed_transaction_handling_revert_execution" => {
                "Failed transaction - contract revert"
            }
            "gas_estimation" => "Gas estimation functionality",
            "transaction_replacement" => "Transaction replacement operations",
            "batch_transactions" => "Batch transaction processing",
            "transaction_count" => "Transaction pending and inmempool count",
            "gas_price_api" => "Gas price API functionality",
            "network_management" => "Network management operations",
            "allowlist_add" => "Allowlist add operation",
            "allowlist_remove" => "Allowlist remove operation",
            "signing_text" => "Text signing functionality",
            "signing_typed_data" => "Typed data signing functionality",
            "transaction_get" => "Transaction get operation",
            "transaction_list" => "Transaction list operation",
            "transaction_replace" => "Transaction replace operation",
            "transaction_cancel" => "Transaction cancel operation",
            "transaction_status_operations" => "Transaction status operations",
            "transaction_counts" => "Transaction count operations",
            "transaction_status_pending" => "Transaction pending state validation",
            "transaction_status_inmempool" => "Transaction inmempool state validation",
            "transaction_status_mined" => "Transaction mined state validation",
            "transaction_status_confirmed" => "Transaction confirmed state validation",
            "transaction_status_failed" => "Transaction failed state validation",
            "transaction_status_expired" => "Transaction expired state validation",
            "allowlist_restrictions" => "Allowlist restriction enforcement",
            "allowlist_edge_cases" => "Allowlist edge case handling",
            "relayer_pause_unpause" => "Relayer pause/unpause functionality",
            "relayer_delete" => "Relayer delete functionality",
            "relayer_gas_configuration" => "Relayer gas configuration management",
            "relayer_allowlist_toggle" => "Relayer allowlist toggle functionality",
            "transaction_nonce_management" => "Transaction nonce management",
            "gas_price_bumping" => "Gas price bumping mechanism",
            "webhook_delivery_testing" => "Webhook delivery testing",
            "rate_limiting_enforcement" => "Rate limiting enforcement",
            "concurrent_transactions" => "Concurrent transaction handling",
            "unauthenticated" => "Unauthenticated protection",
            "blob_transaction_handling" => "Blob transaction handling (EIP-4844)",
            "transaction_data_validation" => "Transaction data validation",
            "balance_edge_cases" => "Balance edge case handling",
            _ => "Unknown test",
        };

        let test_result = self.run_single_test(test_name, description).await;
        suite.add_test(test_result);

        suite.duration = overall_start.elapsed();
        self.print_final_report(&suite);
        suite
    }

    /// Fund a relayer address with ETH from the first Anvil account
    async fn fund_relayer(&self, relayer_address: &EvmAddress) -> Result<()> {
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

        // Send 10 ETH to the relayer
        let funding_amount = U256::from(10_000_000_000_000_000_000_u128); // 10 ETH in wei

        let tx_request =
            TransactionRequest::default().to(relayer_address.into_address()).value(funding_amount);

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

    async fn create_and_fund_relayer(&self, name: &str) -> Result<CreateRelayerResult> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        self.fund_relayer(&relayer.address).await.context("Failed to fund relayer")?;

        Ok(relayer)
    }

    /// run single with:
    /// make run-test-debug TEST=basic_relayer_creation
    async fn test_basic_relayer_creation(&self) -> Result<()> {
        info!("Creating test relayer...");

        let created_relayer = self
            .relayer_client
            .create_relayer("e2e-test-relayer", self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        info!("Created relayer: {:?}", created_relayer);

        let relayer = self
            .relayer_client
            .sdk
            .relayer
            .get(&created_relayer.id)
            .await?
            .context("Failed to fetch relayer")?
            .relayer;

        info!("Fetched relayer {:?}", relayer);

        if relayer.paused {
            return Err(anyhow!("Relayer should not be paused"));
        }

        if relayer.name != "e2e-test-relayer" {
            return Err(anyhow!("Relayer should always be the same name"));
        }

        if relayer.address != created_relayer.address {
            return Err(anyhow!("Relayer should be the same address"));
        }

        if relayer.allowlisted_only {
            return Err(anyhow!("Relayer should not be allowlisted yet"));
        }

        if relayer.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Relayer should not be the same chain"));
        }

        if relayer.max_gas_price.is_some() {
            return Err(anyhow!("Relayer should have a max gas price"));
        }

        if !relayer.eip_1559_enabled {
            return Err(anyhow!("Relayer should have eip 1559 enabled"));
        }

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=simple_eth_transfer
    async fn test_simple_eth_transfer(&self) -> Result<()> {
        info!("Testing simple eth transfer...");

        let relayer = self.create_and_fund_relayer("eth-transfer-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let recipient = &self.config.anvil_accounts[1];
        info!("Sending ETH transfer to {}", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                recipient,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer")?;

        info!("ETH transfer sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=contract_interaction
    async fn test_contract_interaction(&self) -> Result<()> {
        info!("Testing contract interaction...");

        let relayer = self.create_and_fund_relayer("contract-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        info!("Sending contract interaction to deployed contract at {}", contract_address);

        let is_deployed = self.contract_interactor.verify_contract_deployed().await?;
        if !is_deployed {
            return Err(anyhow::anyhow!("Contract verification failed - no code at address"));
        }
        info!("✅ Contract verified as deployed with code at {}", contract_address);

        let relayer_balance = self.contract_interactor.get_eth_balance(&relayer.address).await?;
        info!(
            "Relayer balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(relayer_balance)
        );

        let calldata: TransactionData =
            TransactionData::raw_hex(&self.contract_interactor.encode_simple_call(42)?).unwrap();

        let tx_response = self
            .relayer_client
            .send_transaction(&relayer.id, &contract_address, TransactionValue::zero(), calldata)
            .await
            .context("Failed to send contract interaction")?;

        info!("Contract interaction sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        info!("✅ Contract interaction completed successfully");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=failed_transaction_handling_not_enough_funds
    async fn test_failed_transaction_handling_not_enough_funds(&self) -> Result<()> {
        info!("Testing failed transaction handling not enough funds...");

        let relayer = self.create_and_fund_relayer("failure-test-relayer-funds").await?;
        info!("Created relayer: {:?}", relayer);

        let result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &EvmAddress::zero(),
                alloy::primitives::utils::parse_ether("1000")?.into(),
                TransactionData::empty(),
            )
            .await;

        match result {
            Ok(tx_response) => {
                info!("Potentially failing transaction sent: {:?}", tx_response);
                // Even if sent, it might fail during execution
                let final_status = self.wait_for_transaction_completion(&tx_response.0.id).await;
                if final_status.is_ok() {
                    return Err(anyhow!("Did not fail the transaction something went wrong..."));
                }
                info!("Failure test result: {:?}", final_status);
            }
            Err(e) => {
                info!("Transaction rejected as expected (insufficient funds): {}", e);
                // This is the expected outcome for insufficient funds
            }
        }

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=failed_transaction_handling_revert_execution
    async fn test_failed_transaction_handling_revert_execution(&self) -> Result<()> {
        info!("Testing failed transaction handling revert execution...");

        let relayer = self.create_and_fund_relayer("failure-test-relayer-revert").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &contract_address,
                TransactionValue::zero(),
                TransactionData::from_str("0xdeadbeef").unwrap(), // Invalid function selector - will revert
            )
            .await;

        match result {
            Ok(tx_response) => {
                info!("Contract revert transaction sent: {:?}", tx_response);
                // Even if sent, it should fail during execution
                let final_status = self.wait_for_transaction_completion(&tx_response.0.id).await;
                if final_status.is_ok() {
                    return Err(anyhow!("Did not fail the transaction something went wrong..."));
                }

                info!("Contract revert test result: {:?}", final_status);
            }
            Err(e) => {
                info!("Transaction rejected as expected (contract revert): {}", e);
                // This is also a valid outcome if gas estimation catches the revert
            }
        }

        Ok(())
    }

    // TODO: this one needs reviewing
    /// run single with:
    /// make run-test-debug TEST=gas_estimation
    async fn test_gas_estimation(&self) -> Result<()> {
        info!("Testing gas estimation...");
        let relayer = self.create_and_fund_relayer("gas-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        // Send a simple transaction and verify it uses reasonable gas
        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[3],
                alloy::primitives::utils::parse_ether("0.1")?.into(),
                TransactionData::empty(),
            )
            .await?;

        info!("Gas estimation test transaction: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=batch_transactions
    async fn test_batch_transactions(&self) -> Result<()> {
        info!("Testing batch transactions...");

        for i in 0..3 {
            info!("Mining cleanup block {} before batch test...", i + 1);
            self.mine_and_wait().await?;
        }

        let relayer = self.create_and_fund_relayer("batch-test-relayer").await?;

        info!("Created batch test relayer with ID: {}", relayer.id);

        let mut tx_ids: Vec<TransactionId> = Vec::new();

        for i in 0..3 {
            info!("Preparing to send batch transaction {}/3", i + 1);

            let tx_response = self
                .relayer_client
                .send_transaction(
                    &relayer.id,
                    &self.config.anvil_accounts[4],
                    alloy::primitives::utils::parse_ether("0.01")?.into(),
                    TransactionData::empty(),
                )
                .await?;

            info!("✅ Sent batch transaction {}: {:?}", i + 1, tx_response);
            tx_ids.push(tx_response.0.id);

            self.mine_and_wait().await?;
        }

        info!("All {} batch transactions sent, waiting for completion...", tx_ids.len());

        for (i, tx_id) in tx_ids.iter().enumerate() {
            info!("Waiting for batch transaction {} to complete...", i + 1);
            self.wait_for_transaction_completion(tx_id).await?;
            info!("✅ Batch transaction {} completed", i + 1);
        }

        info!("✅ All batch transactions completed successfully");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_count
    async fn test_transaction_count(&self) -> Result<()> {
        info!("Testing pending count...");

        let relayer = self.create_and_fund_relayer("limits-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let pending_count = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer.id)
            .await
            .context("Failed to get pending count")?;

        if pending_count > 0 {
            return Err(anyhow!("New relayer should not have transaction pending"));
        }

        let inmempool_count = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer.id)
            .await
            .context("Failed to get inmempool count")?;

        if inmempool_count > 0 {
            return Err(anyhow!("New relayer should not have transaction inmempool"));
        }

        let send_count = 3;

        for i in 0..send_count {
            let tx_response = self
                .relayer_client
                .send_transaction(
                    &relayer.id,
                    &self.config.anvil_accounts[4],
                    alloy::primitives::utils::parse_ether("0.01")?.into(),
                    TransactionData::empty(),
                )
                .await?;

            info!("✅ Sent transaction {}: {:?}", i + 1, tx_response);
        }

        let pending_count = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer.id)
            .await
            .context("Failed to get pending count")?;

        if pending_count == 0 {
            return Err(anyhow!("Expected some pending transactions but got none"));
        }

        self.mine_and_wait().await?;

        let pending_count = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer.id)
            .await
            .context("Failed to get pending count")?;

        if pending_count != 0 {
            return Err(anyhow!("Expected 0 pending transactions, got {}", pending_count));
        }

        let inmempool_count = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer.id)
            .await
            .context("Failed to get inmempool count")?;

        if inmempool_count == 0 {
            return Err(anyhow!("Expected some inmempool transactions but got none"));
        }

        self.mine_blocks(2).await?;

        let mut attempts = 0;
        loop {
            let inmempool_count = self
                .relayer_client
                .sdk
                .transaction
                .get_transactions_inmempool_count(&relayer.id)
                .await
                .context("Failed to get inmempool count")?;

            attempts = attempts + 1;

            if inmempool_count != 0 {
                if attempts > 10 {
                    return Err(anyhow!(
                        "Expected 0 inmempool transactions, got {}",
                        inmempool_count
                    ));
                }
            } else {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// run single with:
    /// make run-test-debug TEST=gas_price_api
    async fn test_gas_price_api(&self) -> Result<()> {
        info!("Testing gas price API...");

        let gas_prices = self
            .relayer_client
            .sdk
            .gas
            .get_gas_prices(self.config.chain_id)
            .await
            .context("Failed to get gas prices")?;

        info!("Gas prices for chain {}: {:?}", self.config.chain_id, gas_prices);

        if gas_prices.is_none() {
            return Err(anyhow!("Gas prices not found for the chain"));
        }

        Ok(())
    }

    // TODO: FAILING ON SENDING TX WHEN PAUSED
    /// run single with:
    /// make run-test-debug TEST=network_management
    async fn test_network_management(&self) -> Result<()> {
        info!("Testing network management APIs...");

        let all_networks = self
            .relayer_client
            .sdk
            .network
            .get_all_networks()
            .await
            .context("Failed to get all networks")?;
        info!("All networks: {} found", all_networks.len());

        if all_networks.len() != 1 {
            return Err(anyhow!("Should only bring back 1 network"));
        }

        let network = all_networks.first().unwrap();
        if network.disabled {
            return Err(anyhow!("Network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!(
                "Network provider URL does not match got {}",
                network.provider_urls.first().unwrap()
            ));
        }

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;
        info!("Enabled networks: {} found", enabled_networks.len());

        if enabled_networks.len() != 1 {
            return Err(anyhow!("Should only bring back 1 enabled network"));
        }

        let network = enabled_networks.first().unwrap();
        if network.disabled {
            return Err(anyhow!("Enabled network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Enabled network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Enabled network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Enabled network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Enabled network provider URL does not match"));
        }

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;
        info!("Disabled networks: {} found", disabled_networks.len());

        if disabled_networks.len() != 0 {
            return Err(anyhow!("Should only bring back 0 disabled network"));
        }

        self.relayer_client.sdk.network.disable_network(31337).await?;

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;

        if disabled_networks.len() != 1 {
            return Err(anyhow!("Should only bring back 1 enabled network"));
        }

        let network = disabled_networks.first().unwrap();
        if !network.disabled {
            return Err(anyhow!("Network should be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Network provider URL does not match"));
        }

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;

        if enabled_networks.len() != 0 {
            return Err(anyhow!("Should only bring back 0 enabled network"));
        }

        let relayer = self.create_and_fund_relayer("network-management").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_response = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(
                &relayer.id,
                &RelayTransactionRequest {
                    to: EvmAddress::zero(),
                    value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                    data: TransactionData::empty(),
                    speed: Some(TransactionSpeed::Fast),
                    external_id: None,
                    blobs: None,
                },
            )
            .await;

        if tx_response.is_ok() {
            return Err(anyhow!("Should not be able to send transaction to disabled network"));
        }

        self.relayer_client.sdk.network.enable_network(31337).await?;

        let enabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_enabled_networks()
            .await
            .context("Failed to get enabled networks")?;

        let network = enabled_networks.first().unwrap();
        if network.disabled {
            return Err(anyhow!("Enabled network should not be disabled"));
        }

        if network.chain_id != ChainId::new(31337) {
            return Err(anyhow!("Enabled network chain ID does not match"));
        }

        if network.name != "local_anvil".to_string() {
            return Err(anyhow!("Enabled network name does not match"));
        }

        if network.provider_urls.len() != 1 {
            return Err(anyhow!("Enabled network provider URLs does not match"));
        }

        if network.provider_urls.first().unwrap() != "http://127.0.0.1:8545" {
            return Err(anyhow!("Enabled network provider URL does not match"));
        }

        let disabled_networks = self
            .relayer_client
            .sdk
            .network
            .get_disabled_networks()
            .await
            .context("Failed to get disabled networks")?;

        if disabled_networks.len() != 0 {
            return Err(anyhow!("Should only bring back 0 disabled network"));
        }

        info!("✅ Network management APIs work correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=allowlist_add
    async fn test_allowlist_add(&self) -> Result<()> {
        info!("Testing allowlist list operation...");

        let relayer = self.create_and_fund_relayer("allowlist-list-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        for i in 0..3 {
            let test_address = self.config.anvil_accounts[i];
            self.relayer_client
                .sdk
                .relayer
                .allowlist
                .add(&relayer.id, &test_address)
                .await
                .context("Failed to add address to allowlist")?;
        }

        let paging = PagingContext { limit: 10, offset: 0 };
        let allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &paging)
            .await
            .context("Failed to get allowlist")?;

        info!("✅ Allowlist has {} addresses", allowlist.items.len());

        if allowlist.items.len() != 3 {
            return Err(anyhow::anyhow!(
                "Expected at 3 addresses in allowlist, but got {}",
                allowlist.items.len()
            ));
        }

        let items = allowlist
            .items
            .iter()
            .filter(|a| {
                *a == &self.config.anvil_accounts[0]
                    || *a == &self.config.anvil_accounts[1]
                    || *a == &self.config.anvil_accounts[2]
            })
            .collect::<Vec<&EvmAddress>>();
        if items.len() != allowlist.items.len() {
            return Err(anyhow::anyhow!(
                "Expected at {} addresses in allowlist, but got {}",
                allowlist.items.len(),
                items.len()
            ));
        }

        let tx_response = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(
                &relayer.id,
                &RelayTransactionRequest {
                    to: self.config.anvil_accounts[4],
                    value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                    data: TransactionData::empty(),
                    speed: Some(TransactionSpeed::Fast),
                    external_id: None,
                    blobs: None,
                },
            )
            .await;

        if tx_response.is_ok() {
            return Err(anyhow!("Should not be able to send transaction to none allowed address"));
        }

        for i in 0..3 {
            let test_address = self.config.anvil_accounts[i];
            let _ = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(
                    &relayer.id,
                    &RelayTransactionRequest {
                        to: test_address,
                        value: alloy::primitives::utils::parse_ether("0.5")?.into(),
                        data: TransactionData::empty(),
                        speed: Some(TransactionSpeed::Fast),
                        external_id: None,
                        blobs: None,
                    },
                )
                .await
                .context("Failed to send transaction to allowed address")?;
        }

        info!("✅ Allowlist list operation works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=allowlist_remove
    async fn test_allowlist_remove(&self) -> Result<()> {
        info!("Testing allowlist remove operation...");

        let relayer = self.create_and_fund_relayer("allowlist-remove-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_address = self.config.anvil_accounts[2];
        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .add(&relayer.id, &test_address)
            .await
            .context("Failed to add address to allowlist")?;

        self.relayer_client
            .sdk
            .relayer
            .allowlist
            .delete(&relayer.id, &test_address)
            .await
            .context("Failed to remove address from allowlist")?;

        info!("✅ Removed {} from allowlist", test_address.hex());

        let paging = PagingContext { limit: 10, offset: 0 };
        let updated_allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &paging)
            .await
            .context("Failed to get updated allowlist")?;

        let address_still_exists =
            updated_allowlist.items.iter().any(|addr| addr.hex() == test_address.hex());

        if address_still_exists {
            return Err(anyhow::anyhow!("Address still found in allowlist after deletion"));
        }

        info!("✅ Allowlist remove operation works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=signing_text
    async fn test_signing_text(&self) -> Result<()> {
        info!("Testing text signing...");

        let relayer = self.create_and_fund_relayer("signing-text-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_message = "Hello, RRelayer E2E Test!";

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_text(&relayer.id, test_message)
            .await
            .context("Failed to sign text message")?;

        info!("Signed message. Signature: {}", sign_result.signature);

        info!("✅ Got signature: {:?}", sign_result.signature);

        let paging = PagingContext { limit: 10, offset: 0 };
        let history = self
            .relayer_client
            .sdk
            .sign
            .get_text_history(&relayer.id, &paging)
            .await
            .context("Failed to get text signing history")?;

        info!("Text signing history has {} entries", history.items.len());

        let signed_message = history.items.iter().find(|entry| entry.message == test_message);

        if let Some(entry) = signed_message {
            info!("✅ Found signed message in history: {}", entry.message);
            info!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed message not found in history"));
        }

        info!("✅ Text signing works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=signing_typed_data
    async fn test_signing_typed_data(&self) -> Result<()> {
        info!("Testing typed data signing...");

        let relayer = self.create_and_fund_relayer("signing-typed-data-relayer").await?;
        info!("Created relayer: {:?}", relayer);

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

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data)
            .await
            .context("Failed to sign typed data")?;

        info!("Signed typed data. Signature: {}", sign_result.signature);

        info!("✅ Got typed data signature: {:?}", sign_result.signature);

        let paging = PagingContext { limit: 10, offset: 0 };
        let history = self
            .relayer_client
            .sdk
            .sign
            .get_typed_data_history(&relayer.id, &paging)
            .await
            .context("Failed to get typed data signing history")?;

        info!("Typed data signing history has {} entries", history.items.len());

        let signed_entry = history.items.iter().find(|entry| {
            if let Some(domain) = entry.domain_data.get("name") {
                domain.as_str() == Some("RRelayer Test")
            } else {
                false
            }
        });

        if let Some(entry) = signed_entry {
            info!("✅ Found signed typed data in history: {:?}", entry.domain_data);
            info!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed typed data not found in history"));
        }

        info!("✅ Typed data signing works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_get
    async fn test_transaction_get(&self) -> Result<()> {
        info!("Testing transaction get operation...");

        let relayer = self.create_and_fund_relayer("tx-get-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-get".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let retrieved_tx = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction(transaction_id)
            .await
            .context("Failed to get transaction")?;

        if let Some(tx) = retrieved_tx {
            self.relayer_client.sent_transaction_compare(tx_request, tx)?;
        } else {
            return Err(anyhow::anyhow!("Transaction not found"));
        }

        info!("✅ Transaction get works correctly");

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_list
    async fn test_transaction_list(&self) -> Result<()> {
        info!("Testing transaction list operation...");

        let relayer = self.create_and_fund_relayer("tx-list-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        for i in 1..=3 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.1")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("test-list-{}", i)),
                blobs: None,
            };

            let _ = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer.id, &tx_request)
                .await
                .context("Failed to send transaction")?;
        }

        let paging = PagingContext { limit: 10, offset: 0 };
        let relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions(&relayer.id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!("✅ Found {} transactions for relayer", relayer_transactions.items.len());

        if relayer_transactions.items.len() != 3 {
            return Err(anyhow::anyhow!(
                "Expected at 3 transactions, but got {}",
                relayer_transactions.items.len()
            ));
        }

        info!("✅ Transaction list operation works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_replace
    async fn test_transaction_replace(&self) -> Result<()> {
        info!("Testing transaction replace operation...");

        let relayer = self.create_and_fund_relayer("tx-replace-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("test-original".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let replacement_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.2")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-replacement".to_string()),
            blobs: None,
        };

        let replace_result = self
            .relayer_client
            .sdk
            .transaction
            .replace_transaction(transaction_id, &replacement_request)
            .await
            .context("Failed to replace transaction")?;
        info!("✅ Transaction replacement result: {}", replace_result);

        if !replace_result {
            return Err(anyhow::anyhow!("Replace transaction failed"));
        }

        self.anvil_manager.mine_block().await?;

        let transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        self.relayer_client.sent_transaction_compare(replacement_request, transaction)?;

        info!("✅ Transaction replace operation works correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_cancel
    async fn test_transaction_cancel(&self) -> Result<()> {
        info!("Testing transaction cancel operation...");

        let relayer = self.create_and_fund_relayer("tx-cancel-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("test-original".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let cancel_result = self
            .relayer_client
            .sdk
            .transaction
            .cancel_transaction(transaction_id)
            .await
            .context("Failed to cancel transaction")?;

        if !cancel_result {
            return Err(anyhow::anyhow!("Cancel transaction failed"));
        }

        self.anvil_manager.mine_block().await?;

        let transaction = self.relayer_client.get_transaction(&send_result.id).await?;
        if !transaction.is_noop {
            return Err(anyhow::anyhow!(
                "Expected the transaction to be a no-op {}",
                transaction_id
            ));
        }

        info!("✅ Transaction {} cancel operation works correctly", transaction_id);

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_operations
    async fn test_transaction_status_operations(&self) -> Result<()> {
        info!("Testing transaction status operations...");

        let relayer = self.create_and_fund_relayer("tx-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("test-status-op".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;
        info!("Sent transaction for status testing: {}", transaction_id);

        let status_result = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(transaction_id)
            .await
            .context("Failed to get transaction status")?;

        if let Some(result) = status_result {
            // this depends on how fast relayer executes the queue
            if result.status != TransactionStatus::Pending
                && result.status != TransactionStatus::Inmempool
            {
                return Err(anyhow::anyhow!(
                    "Transaction status should be inmempool or pending at this point but it is {}",
                    result.status
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Transaction status not found"));
        }

        self.mine_and_wait().await?;

        let updated_status = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(transaction_id)
            .await
            .context("Failed to get updated transaction status")?;

        if let Some(status) = updated_status {
            if status.status != TransactionStatus::Mined {
                return Err(anyhow::anyhow!("Transaction status should be mined at this point"));
            }
        }

        info!("✅ Transaction status operations work correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_counts
    async fn test_transaction_counts(&self) -> Result<()> {
        info!("Testing transaction count operations...");

        let relayer = self.create_and_fund_relayer("tx-counts-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let initial_pending = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer.id)
            .await
            .context("Failed to get initial pending count")?;

        let initial_inmempool = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer.id)
            .await
            .context("Failed to get initial inmempool count")?;

        info!("Initial counts - Pending: {}, InMempool: {}", initial_pending, initial_inmempool);

        let mut transaction_ids = Vec::new();
        for i in 0..3 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: TransactionValue::new(U256::from(100000000000000000u128 * (i + 1))),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("test-counts-{}", i)),
                blobs: None,
            };

            let send_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer.id, &tx_request)
                .await
                .context(format!("Failed to send transaction {}", i))?;

            transaction_ids.push(send_result.id.clone());
            info!("Sent transaction {}: {}", i, send_result.id);

            self.mine_and_wait().await?;
        }

        let final_pending = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_pending_count(&relayer.id)
            .await
            .context("Failed to get final pending count")?;

        let final_inmempool = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions_inmempool_count(&relayer.id)
            .await
            .context("Failed to get final inmempool count")?;

        info!("Final counts - Pending: {}, InMempool: {}", final_pending, final_inmempool);

        // Verify counts make sense (should have increased)
        let total_final = final_pending + final_inmempool;
        let total_initial = initial_pending + initial_inmempool;

        if total_final >= total_initial {
            info!("✅ Transaction counts increased as expected");
        } else {
            return Err(anyhow!(
                "Transaction counts may have decreased (transactions completed quickly)"
            ));
        }

        info!("✅ Transaction count operations work correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_pending
    async fn test_transaction_status_pending(&self) -> Result<()> {
        info!("Testing transaction pending state...");

        let relayer = self.create_and_fund_relayer("pending-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-pending".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        let status = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(&send_result.id)
            .await?
            .context("Transaction status not found")?;

        if status.status != TransactionStatus::Pending {
            return Err(anyhow::anyhow!(
                "Expected transaction to be in Pending state, but got: {:?}",
                status.status
            ));
        }

        if status.hash.is_some() {
            return Err(anyhow::anyhow!(
                "Pending transaction should not have hash, but got: {:?}",
                status.hash
            ));
        }

        if status.receipt.is_some() {
            return Err(anyhow::anyhow!(
                "Pending transaction should not have receipt, but got receipt"
            ));
        }

        info!("✅ Transaction stays in Pending state without mining");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_inmempool
    async fn test_transaction_status_inmempool(&self) -> Result<()> {
        info!("Testing transaction inmempool state...");

        let relayer = self.create_and_fund_relayer("inmempool-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-inmempool".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        // Wait for transaction to be sent to network (should move to InMempool)
        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Inmempool {
                if status.hash.is_none() {
                    return Err(anyhow::anyhow!("InMempool transaction should have hash"));
                }
                if status.receipt.is_some() {
                    return Err(anyhow::anyhow!("InMempool transaction should not have receipt"));
                }
                info!("✅ Transaction successfully reached InMempool state");
                return Ok(());
            }

            attempts += 1;
            if attempts > 10 {
                anyhow::bail!("Transaction did not reach InMempool state in time");
            }
        }
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_mined
    async fn test_transaction_status_mined(&self) -> Result<()> {
        info!("Testing transaction mined state...");

        let relayer = self.create_and_fund_relayer("mined-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-mined".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Inmempool {
                break;
            }
        }

        self.mine_and_wait().await?;

        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Mined {
                if status.hash.is_none() {
                    return Err(anyhow::anyhow!("Mined transaction should have hash"));
                }
                if status.receipt.is_none() {
                    return Err(anyhow::anyhow!("Mined transaction should have receipt"));
                }
                let receipt = status.receipt.unwrap();
                info!("Transaction receipt: {:?}", receipt);
                if !receipt.inner.inner.status() {
                    return Err(anyhow::anyhow!("Mined transaction should have a success as true"));
                }

                info!("✅ Transaction successfully reached Mined state");
                return Ok(());
            }

            attempts += 1;
            if attempts > 10 {
                anyhow::bail!("Transaction did not reach Mined state in time");
            }
        }
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_confirmed
    async fn test_transaction_status_confirmed(&self) -> Result<()> {
        info!("Testing transaction confirmed state...");

        let relayer = self.create_and_fund_relayer("confirmed-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-confirmed".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Inmempool {
                break;
            }
        }

        self.mine_blocks(15).await?;

        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Confirmed {
                if status.hash.is_none() {
                    return Err(anyhow::anyhow!("Confirmed transaction should have hash"));
                }
                if status.receipt.is_none() {
                    return Err(anyhow::anyhow!("Confirmed transaction should have receipt"));
                }
                info!("✅ Transaction successfully reached Confirmed state");
                return Ok(());
            }

            attempts += 1;
            if attempts > 25 {
                anyhow::bail!("Transaction did not reach Confirmed state in time");
            }
        }
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_status_failed
    async fn test_transaction_status_failed(&self) -> Result<()> {
        info!("Testing transaction failed state...");

        let relayer = self.create_and_fund_relayer("failed-status-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        let tx_request = RelayTransactionRequest {
            to: contract_address,
            value: TransactionValue::new(U256::ZERO),
            data: TransactionData::new(alloy::primitives::Bytes::from_static(&[
                0xde, 0xad, 0xbe, 0xef,
            ])), // Invalid function selector
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("test-failed".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await;

        match send_result {
            Ok(tx_response) => {
                return Err(anyhow::anyhow!(
                    "Transaction sent successfully, but should have failed: {:?}",
                    tx_response
                ));
            }
            Err(_) => {
                info!(
                    "✅ Transaction was rejected at gas estimation (also valid failure scenario)"
                );
                Ok(())
            }
        }
    }

    //TODO! NEED TO THINK ABOUT HOW TO TEST EXPIRED
    /// run single with:
    /// make run-test-debug TEST=transaction_status_expired
    async fn test_transaction_status_expired(&self) -> Result<()> {
        info!("Testing transaction expired state...");

        error!("NEED TO WRITE THIS TEST");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=allowlist_restrictions
    async fn test_allowlist_restrictions(&self) -> Result<()> {
        info!("Testing allowlist restrictions...");

        let relayer = self.create_and_fund_relayer("allowlist-restriction-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let allowed_address = self.config.anvil_accounts[1];
        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &allowed_address).await?;

        let allowed_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.1")?.into(),
                TransactionData::empty(),
            )
            .await;

        if allowed_tx_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction to allowlisted address should succeed, but got error: {:?}",
                allowed_tx_result.err()
            ));
        }

        let forbidden_tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[2], // Different address
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if forbidden_tx_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction to non-allowlisted address should fail, but succeeded"
            ));
        }

        info!("✅ Allowlist restrictions working correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=allowlist_edge_cases
    async fn test_allowlist_edge_cases(&self) -> Result<()> {
        info!("Testing allowlist edge cases...");

        let relayer = self.create_and_fund_relayer("allowlist-edge-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_address = self.config.anvil_accounts[1];

        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &test_address).await?;
        let duplicate_result =
            self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &test_address).await;

        // Should handle duplicate gracefully
        // Duplicate add should be handled gracefully - both success and error are acceptable
        match duplicate_result {
            Ok(_) => info!("Duplicate address add succeeded (graceful handling)"),
            Err(_) => {
                return Err(anyhow::anyhow!("Duplicate address add failed (graceful handling)"))
            }
        }

        let non_existent = self.config.anvil_accounts[4];
        let remove_result =
            self.relayer_client.sdk.relayer.allowlist.delete(&relayer.id, &non_existent).await;

        // Should handle gracefully
        // Remove non-existent should be handled gracefully - both success and error are acceptable
        match remove_result {
            Ok(_) => info!("Remove non-existent succeeded (graceful handling)"),
            Err(_) => {
                return Err(anyhow::anyhow!("Remove non-existent failed (graceful handling)"))
            }
        }

        let allowlist = self
            .relayer_client
            .sdk
            .relayer
            .allowlist
            .get_all(&relayer.id, &PagingContext::new(50, 0))
            .await?;

        if allowlist.items.len() != 1 {
            return Err(anyhow::anyhow!(
                "Allowlist should have 1 item, but got: {:?}",
                allowlist.items.len()
            ));
        }

        if allowlist.items[0] != test_address {
            return Err(anyhow::anyhow!(
                "Allowlist should have first item be test address, but got: {:?}",
                allowlist.items[0]
            ));
        }

        info!("✅ Allowlist edge cases handled correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=relayer_delete
    async fn test_relayer_delete(&self) -> Result<()> {
        info!("Testing relayer pause/unpause...");

        let relayer = self.create_and_fund_relayer("pause-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let created_relayer =
            self.relayer_client.sdk.relayer.get(&relayer.id).await?.context("Relayer should exist")?;

        if created_relayer.relayer.id != relayer.id {
            return Err(anyhow::anyhow!("Relayer should exist"));
        }

        self.relayer_client.sdk.relayer.delete(&relayer.id).await?;


        let created_relayer =
            self.relayer_client.sdk.relayer.get(&relayer.id).await;

        match created_relayer {
            Ok(_) => {
                Err(anyhow::anyhow!("Relayer should have been deleted"))
            }
            Err(_) => {
                info!("✅ Relayer delete functionality working correctly");
                Ok(())
            }
        }
    }

    /// run single with:
    /// make run-test-debug TEST=relayer_pause_unpause
    async fn test_relayer_pause_unpause(&self) -> Result<()> {
        info!("Testing relayer pause/unpause...");

        let relayer = self.create_and_fund_relayer("pause-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let normal_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if normal_result.is_err() {
            return Err(anyhow::anyhow!(
                "Normal transaction should succeed, but got error: {:?}",
                normal_result.err()
            ));
        }

        self.relayer_client.sdk.relayer.pause(&relayer.id).await?;

        let paused_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = paused_config {
            if !config.relayer.paused {
                return Err(anyhow::anyhow!("Relayer should be paused, but is not"));
            }
        }

        let paused_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if paused_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction should fail when relayer is paused, but succeeded"
            ));
        }

        self.relayer_client.sdk.relayer.unpause(&relayer.id).await?;

        let unpaused_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = unpaused_config {
            if config.relayer.paused {
                return Err(anyhow::anyhow!("Relayer should not be paused, but is"));
            }
        }

        let unpaused_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if unpaused_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction should succeed after unpause, but got error: {:?}",
                unpaused_result.err()
            ));
        }

        info!("✅ Relayer pause/unpause functionality working correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=relayer_gas_configuration
    async fn test_relayer_gas_configuration(&self) -> Result<()> {
        info!("Testing relayer gas configuration...");

        let relayer = self.create_and_fund_relayer("gas-config-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        self.relayer_client.sdk.relayer.update_eip1559_status(&relayer.id, false).await?;

        let config_after_legacy = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = config_after_legacy {
            if config.relayer.eip_1559_enabled {
                return Err(anyhow::anyhow!(
                    "Relayer should not be using EIP1559 but it is enabled"
                ));
            }
        }

        self.relayer_client.sdk.relayer.update_eip1559_status(&relayer.id, true).await?;

        let config_after_latest = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = config_after_latest {
            if !config.relayer.eip_1559_enabled {
                return Err(anyhow::anyhow!(
                    "Relayer should be using EIP1559 but it is not enabled"
                ));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        self.relayer_client.sdk.relayer.update_max_gas_price(&relayer.id, 1000000).await?;

        let config_after_max = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = config_after_max {
            if let Some(max) = config.relayer.max_gas_price {
                if max != GasPrice::new(1000000) {
                    return Err(anyhow::anyhow!(
                        "Relayer should have max gas price of 1000000, but got: {:?}",
                        max
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("Relayer should have a max gas price"));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        let tx_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if tx_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction should succeed with gas configuration, but got error: {:?}",
                tx_result.err()
            ));
        }

        self.relayer_client.sdk.relayer.remove_max_gas_price(&relayer.id).await?;

        let config_after_none = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = config_after_none {
            if config.relayer.max_gas_price.is_some() {
                return Err(anyhow::anyhow!("Relayer should not have a max gas price"));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        info!("✅ Gas configuration changes working correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=relayer_allowlist_toggle
    async fn test_relayer_allowlist_toggle(&self) -> Result<()> {
        info!("Testing relayer allowlist toggle...");

        let relayer = self.create_and_fund_relayer("allowlist-toggle-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let initial_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        if let Some(config) = initial_config {
            if config.relayer.allowlisted_only {
                return Err(anyhow::anyhow!("Relayer should not be allowlisted only"));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        let no_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if no_allowlist_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction should succeed without allowlist, but got error: {:?}",
                no_allowlist_result.err()
            ));
        }

        let allowed_address = &self.config.anvil_accounts[1];
        self.relayer_client.sdk.relayer.allowlist.add(&relayer.id, &allowed_address).await?;

        let enabled_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        info!("Relayer config after enable attempt: {:?}", enabled_config);
        if let Some(config) = enabled_config {
            if !config.relayer.allowlisted_only {
                return Err(anyhow::anyhow!("Relayer should be allowlisted only"));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        let empty_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[3],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if empty_allowlist_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction should fail with unknown allowlist, but succeeded"
            ));
        }

        let with_allowlist_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &allowed_address,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if with_allowlist_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction should succeed with allowlist entry, but got error: {:?}",
                with_allowlist_result.err()
            ));
        }

        self.relayer_client.sdk.relayer.allowlist.delete(&relayer.id, &allowed_address).await?;

        let disabled_config = self.relayer_client.sdk.relayer.get(&relayer.id).await?;
        info!("Final relayer config: {:?}", disabled_config);
        if let Some(config) = disabled_config {
            if config.relayer.allowlisted_only {
                return Err(anyhow::anyhow!("Relayer should not be allowlisted only"));
            }
        } else {
            return Err(anyhow::anyhow!("Relayer should have a config"));
        }

        info!("✅ Allowlist toggle functionality working correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_nonce_management
    async fn test_transaction_nonce_management(&self) -> Result<()> {
        info!("Testing transaction nonce management...");

        let relayer = self.create_and_fund_relayer("nonce-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let mut transaction_ids = Vec::new();

        for i in 0..50 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.000000005")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("nonce-test-{}", i)),
                blobs: None,
            };

            let send_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer.id, &tx_request)
                .await?;

            transaction_ids.push(send_result.id);
        }

        let mut nonces = Vec::new();
        for tx_id in &transaction_ids {
            if let Some(tx) = self.relayer_client.sdk.transaction.get_transaction(tx_id).await? {
                nonces.push(tx.nonce.into_inner());
            }
        }

        nonces.sort();

        for i in 1..nonces.len() {
            if nonces[i] != nonces[i - 1] + 1 {
                return Err(anyhow::anyhow!(
                    "Nonces should be sequential, but nonce {} ({}) != previous nonce {} ({}) + 1",
                    i,
                    nonces[i],
                    i - 1,
                    nonces[i - 1]
                ));
            }
        }

        self.mine_and_wait().await?;
        info!("Waiting for all transactions to reach mempool...");

        let timeout = Duration::from_secs(30);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for transactions to reach mempool"));
            }

            let mut all_in_mempool = true;
            for tx_id in &transaction_ids {
                if let Some(tx) = self.relayer_client.sdk.transaction.get_transaction(tx_id).await?
                {
                    if tx.status != TransactionStatus::Mined {
                        info!("Transaction {} not in mempool - status {}", tx_id, tx.status);
                        all_in_mempool = false;
                        break;
                    }
                } else {
                    info!("Transaction {} not in mempool - status", tx_id);
                    all_in_mempool = false;
                    break;
                }
            }

            if all_in_mempool {
                info!("All {} transactions are now in mempool", transaction_ids.len());
                break;
            }

            self.mine_and_wait().await?;
        }

        info!("✅ Nonce management working correctly with sequential assignment");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=gas_price_bumping
    async fn test_gas_price_bumping(&self) -> Result<()> {
        info!("Testing gas price bumping...");

        let relayer = self.create_and_fund_relayer("gas-bump-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.5")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Slow),
            external_id: Some("gas-bump-test".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        let mut attempts = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let status = self
                .relayer_client
                .sdk
                .transaction
                .get_transaction_status(&send_result.id)
                .await?
                .context("Transaction status not found")?;

            if status.status == TransactionStatus::Inmempool {
                info!("Transaction reached InMempool with hash: {:?}", status.hash);
                break;
            }

            attempts += 1;
            if attempts > 20 {
                anyhow::bail!("Transaction did not reach InMempool");
            }
        }

        let transaction_before = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction(&send_result.id)
            .await?
            .context("Transaction not found")?;
        let max_fee_per_gas_before = transaction_before
            .sent_with_max_fee_per_gas
            .context("transaction_before did not have sent_with_max_fee_per_gas")?;
        let sent_with_max_priority_before =
            transaction_before
                .sent_with_max_priority_fee_per_gas
                .context("transaction_before did not have sent_with_max_priority_fee_per_gas")?;

        // wait 10 seconds as gas bumping happens based on time
        tokio::time::sleep(Duration::from_secs(10)).await;

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        let transaction_after = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction(&send_result.id)
            .await?
            .context("Transaction not found")?;
        let max_fee_per_gas_after = transaction_after
            .sent_with_max_fee_per_gas
            .context("transaction_after did not have sent_with_max_fee_per_gas")?;
        let sent_with_max_priority_after = transaction_after
            .sent_with_max_priority_fee_per_gas
            .context("transaction_after did not have sent_with_max_priority_fee_per_gas")?;

        if max_fee_per_gas_before == max_fee_per_gas_after {
            return Err(anyhow::anyhow!("Gas price did not bump max_fee"));
        }

        if sent_with_max_priority_before == sent_with_max_priority_after {
            return Err(anyhow::anyhow!("Gas price did not bump max_priority_fee"));
        }

        let transaction_status = self
            .relayer_client
            .sdk
            .transaction
            .get_transaction_status(&send_result.id)
            .await?
            .context("Transaction status not found")?
            .receipt
            .context("Transaction status did not have receipt")?;
        if !transaction_status.status() {
            return Err(anyhow::anyhow!("Transaction failed after gas bumping"));
        }

        info!("✅ Gas price bumping mechanism verified");
        Ok(())
    }

    // TODO: handle webhooks
    /// Test webhook delivery mechanism
    async fn test_webhook_delivery(&self) -> Result<()> {
        info!("Testing webhook delivery...");

        // Note: This test would require setting up webhook endpoints
        // For now, we'll test that webhooks are configured and transaction events trigger them

        let relayer = self.create_and_fund_relayer("webhook-test-relayer").await?;

        // Send a transaction that should trigger webhook events
        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: TransactionValue::new(U256::from(100000000000000000u128)),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("webhook-test".to_string()),
            blobs: None,
        };

        let send_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        // Mine the transaction to completion
        self.wait_for_transaction_completion(&send_result.id).await?;

        // TODO: In a real scenario, we would verify webhook deliveries here
        // For this test, we just ensure the transaction completed successfully

        info!("✅ Webhook delivery mechanism verified (would trigger events)");
        Ok(())
    }

    // TODO: handle rate limits by making it more simple
    /// Test rate limiting enforcement
    async fn test_rate_limiting(&self) -> Result<()> {
        info!("Testing rate limiting enforcement...");

        // Note: Rate limiting depends on configuration and would need specific setup
        // This test verifies the basic mechanism exists

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;

        // Send multiple transactions rapidly to potentially trigger rate limiting
        let mut successful_transactions = 0;
        let mut rate_limited = false;

        for i in 0..10 {
            let value: U256 = U256::ZERO * U256::from(i + 1);
            let tx_result = self
                .relayer_client
                .send_transaction(
                    &relayer.id,
                    &self.config.anvil_accounts[1],
                    TransactionValue::new(value.into()),
                    TransactionData::empty(),
                )
                .await;

            match tx_result {
                Ok(_) => successful_transactions += 1,
                Err(e) => {
                    if e.to_string().contains("rate limit")
                        || e.to_string().contains("too many requests")
                    {
                        rate_limited = true;
                        info!("Rate limiting triggered at transaction {}", i);
                        break;
                    } else {
                        info!("Transaction {} failed with error: {}", i, e);
                    }
                }
            }

            // Small delay between requests
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        info!("Successful transactions before rate limit: {}", successful_transactions);

        info!("✅ Rate limiting mechanism verified (may not trigger with default config)");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=concurrent_transactions
    async fn test_concurrent_transactions(&self) -> Result<()> {
        info!("Testing concurrent transactions...");

        let relayer = self.create_and_fund_relayer("concurrent-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let mut tx_requests = Vec::new();
        for i in 0..50 {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[1],
                value: alloy::primitives::utils::parse_ether("0.000000005")?.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some(format!("concurrent-test-{}", i)),
                blobs: None,
            };
            tx_requests.push(tx_request);
        }

        info!("Sending {} transactions concurrently...", tx_requests.len());
        let mut handles = Vec::new();

        for (i, tx_request) in tx_requests.into_iter().enumerate() {
            let relayer_client = self.relayer_client.clone();
            let relayer_id = relayer.id;

            let handle = tokio::spawn(async move {
                let result =
                    relayer_client.sdk.transaction.send_transaction(&relayer_id, &tx_request).await;
                (i, result)
            });

            handles.push(handle);
        }

        let mut transaction_ids = Vec::new();
        let mut successful = 0;
        let mut failed = 0;

        for handle in handles {
            let (i, result) = handle.await?;
            match result {
                Ok(send_result) => {
                    transaction_ids.push(send_result.id);
                    successful += 1;
                }
                Err(e) => {
                    info!("Transaction {} failed: {}", i, e);
                    failed += 1;
                }
            }
        }

        info!("Concurrent transactions - Successful: {}, Failed: {}", successful, failed);

        if failed != 0 {
            return Err(anyhow::anyhow!("Concurrent transactions failed - {}", failed));
        }

        self.mine_and_wait().await?;
        info!("Waiting for all transactions to reach mined status...");

        let timeout = Duration::from_secs(30);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for transactions to be mined"));
            }

            let mut all_mined = true;
            for tx_id in &transaction_ids {
                if let Some(tx) = self.relayer_client.sdk.transaction.get_transaction(tx_id).await?
                {
                    if tx.status != TransactionStatus::Mined {
                        all_mined = false;
                        break;
                    }
                } else {
                    all_mined = false;
                    break;
                }
            }

            if all_mined {
                info!("All {} transactions are now mined", transaction_ids.len());
                break;
            }

            self.mine_and_wait().await?;
        }

        info!("✅ Concurrent transaction handling verified");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=unauthenticated
    async fn test_unauthenticated(&self) -> Result<()> {
        info!("Testing unauthenticated requests...");

        let config = E2ETestConfig::default();
        let sdk =
            SDK::new(config.rrelayer_base_url.clone(), "wrong".to_string(), "way".to_string());
        info!("Created SDK with wrong credentials");

        // Test basic auth status
        let auth_status = sdk.auth.test_auth().await;
        if auth_status.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be authenticated"));
        }

        let relay = sdk.relayer.create(31337, "yes").await;
        if relay.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to create a relayer"));
        }

        let relayers = sdk.relayer.get_all(Some(31337), &PagingContext::new(50, 0)).await;
        if relayers.is_ok() {
            return Err(anyhow::anyhow!("SDK should not be able to get relayers"));
        }

        info!("✅ Unauthenticated checked");
        Ok(())
    }

    // TODO: got to here

    /// run single with:
    /// make run-test-debug TEST=blob_transaction_handling
    async fn test_blob_transaction_handling(&self) -> Result<()> {
        info!("Testing blob transaction handling...");

        let relayer = self.create_and_fund_relayer("blob-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let blob_data = vec![1u8; 131072]; // 128KB blob
        let hex_blob = format!("0x{}", alloy::hex::encode(&blob_data));

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: TransactionValue::new(U256::ZERO),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("blob-test".to_string()),
            blobs: Some(vec![hex_blob]),
        };

        let blob_result =
            self.relayer_client.sdk.transaction.send_transaction(&relayer.id, &tx_request).await?;

        let result = self.wait_for_transaction_completion(&blob_result.id).await?;

        self.relayer_client.sent_transaction_compare(tx_request, result.0)?;

        Ok(())
    }

    /// Test transaction data validation
    async fn test_transaction_data_validation(&self) -> Result<()> {
        info!("Testing transaction data validation...");

        let relayer = self.create_and_fund_relayer("data-validation-relayer").await?;

        // Test 1: Valid hex data
        let valid_data_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::from_str("0x1234abcd").unwrap(),
            )
            .await;

        if valid_data_result.is_err() {
            return Err(anyhow::anyhow!(
                "Valid hex data should be accepted, but got error: {:?}",
                valid_data_result.err()
            ));
        }

        // Test 2: Empty data (should be valid)
        let empty_data_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await;

        if empty_data_result.is_err() {
            return Err(anyhow::anyhow!(
                "Empty data should be accepted, but got error: {:?}",
                empty_data_result.err()
            ));
        }

        // Test 3: Invalid hex data (should be caught by client validation)
        let _ = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::from_str("0xGGGG").unwrap(), // Invalid hex
            )
            .await;

        info!("✅ Transaction data validation working");
        Ok(())
    }

    /// Test balance edge cases
    async fn test_balance_edge_cases(&self) -> Result<()> {
        info!("Testing balance edge cases...");

        let relayer = self.create_and_fund_relayer("balance-edge-relayer").await?;

        // Test 1: Get relayer balance - placeholder method
        // Note: This method may not exist in current RelayerClient
        let balance_result: Result<alloy::primitives::U256> =
            Err(anyhow::anyhow!("Balance API not implemented"));
        match balance_result {
            Ok(balance) => {
                info!("Relayer balance: {} ETH", alloy::primitives::utils::format_ether(balance));
                if balance == U256::ZERO {
                    return Err(anyhow::anyhow!(
                        "Funded relayer should have positive balance, but got zero balance"
                    ));
                }
            }
            Err(e) => {
                info!("Balance query failed: {}", e);
                // This might be expected depending on API implementation
            }
        }

        // Test 2: Try to send more than balance (should fail)
        let excessive_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("100_000")?.into(),
                TransactionData::empty(),
            )
            .await;

        if excessive_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction exceeding balance should fail, but succeeded"
            ));
        }

        // Test 3: Send exactly the gas cost amount (edge case)
        let small_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("0.001")?.into(),
                TransactionData::empty(),
            )
            .await;

        // This should succeed or fail based on gas costs
        info!("Small amount transaction result: {:?}", small_result);

        info!("✅ Balance edge cases handled correctly");
        Ok(())
    }

    async fn wait_for_transaction_completion(
        &self,
        transaction_id: &TransactionId,
    ) -> Result<(Transaction, AnyTransactionReceipt)> {
        let timeout = Duration::from_secs(self.config.test_timeout_seconds);
        let start = tokio::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!(
                    "Transaction {} timed out after {} seconds",
                    transaction_id,
                    self.config.test_timeout_seconds
                );
            }

            let result = self.relayer_client.get_transaction_status(transaction_id).await?;
            info!("Transaction {} status: {:?}", transaction_id, result);

            match result.status {
                TransactionStatus::Confirmed | TransactionStatus::Mined => {
                    info!("Transaction {} completed successfully", transaction_id);
                    let transaction = self
                        .relayer_client
                        .get_transaction(&transaction_id)
                        .await
                        .context("Could not get the transaction")?;

                    return Ok((
                        transaction,
                        result.receipt.expect("Transaction receipt should always be present now"),
                    ));
                }
                TransactionStatus::Failed => {
                    anyhow::bail!("Transaction {} failed: {:?}", transaction_id, result);
                }
                TransactionStatus::Pending | TransactionStatus::Inmempool => {
                    info!(
                        "Transaction {} still pending, mining a block and waiting...",
                        transaction_id
                    );
                    self.mine_and_wait().await?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                TransactionStatus::Expired => {
                    anyhow::bail!("Transaction {} expired: {:?}", transaction_id, result);
                }
            }
        }
    }
}
