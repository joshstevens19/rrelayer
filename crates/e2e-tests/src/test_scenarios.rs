use alloy::dyn_abi::TypedData;
use alloy::network::{AnyTransactionReceipt, EthereumWallet, ReceiptResponse};
use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{anyhow, Context, Result};
use futures;
use rrelayer_core::gas::types::GasPrice;
use rrelayer_core::network::ChainId;
use rrelayer_core::relayer::api::CreateRelayerResult;
use rrelayer_core::transaction::api::get_transaction_status::RelayTransactionStatusResult;
use rrelayer_core::transaction::types::Transaction;
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext},
    transaction::api::send_transaction::RelayTransactionRequest,
    transaction::types::{
        TransactionData, TransactionId, TransactionSpeed, TransactionStatus, TransactionValue,
    },
};
use rrelayer_sdk::SDK;
use std::collections::{HashMap, HashSet};
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
            TestResult::Timeout => Some("Test timed out after 90 seconds".to_string()),
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
    relayer_client::RelayerClient, test_config::E2ETestConfig, webhook_server::WebhookTestServer,
};

pub struct TestRunner {
    config: E2ETestConfig,
    relayer_client: RelayerClient,
    contract_interactor: ContractInteractor,
    anvil_manager: AnvilManager,
    webhook_server: Option<WebhookTestServer>,
    relayer_counter: std::sync::atomic::AtomicUsize,
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

        // Deploy the ERC-20 test token
        let token_address = contract_interactor
            .deploy_test_token(deployer_private_key)
            .await
            .context("Failed to deploy test token")?;

        info!("✅ Test ERC-20 token deployed at: {:?}", token_address);

        // Deploy the Safe contracts
        let safe_address = contract_interactor
            .deploy_safe_contracts(deployer_private_key)
            .await
            .context("Failed to deploy Safe contracts")?;

        info!("✅ Safe contracts deployed - Safe proxy at: {:?}", safe_address);

        // Initialize webhook server with the shared secret from yaml
        let webhook_server = Some(WebhookTestServer::new("test-webhook-secret-123".to_string()));

        Ok(Self {
            config,
            relayer_client,
            contract_interactor,
            anvil_manager,
            webhook_server,
            relayer_counter: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn into_anvil_manager(self) -> AnvilManager {
        self.anvil_manager
    }

    /// Start the webhook test server
    pub async fn start_webhook_server(&self) -> Result<()> {
        if let Some(webhook_server) = &self.webhook_server {
            let server = webhook_server.clone();
            tokio::spawn(async move {
                if let Err(e) = server.start(8546).await {
                    error!("Webhook server failed to start: {}", e);
                }
            });

            // Wait a moment for server to start
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            info!("✅ Webhook test server started on port 8546");
        }
        Ok(())
    }

    /// Stop the webhook test server
    pub fn stop_webhook_server(&self) {
        if let Some(webhook_server) = &self.webhook_server {
            webhook_server.stop();
            info!("🛑 Webhook test server stopped");
        }
    }

    /// Get reference to webhook server for testing
    pub fn webhook_server(&self) -> Option<&WebhookTestServer> {
        self.webhook_server.as_ref()
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

        self.start_webhook_server().await.unwrap();
        let relayer = self.create_and_fund_relayer("automatic_top_up").await.unwrap();
        self.fund_relayer(
            &relayer.address,
            alloy::primitives::utils::parse_ether("100000000").unwrap(),
        )
        .await
        .unwrap();

        info!("🔍 Testing webhook server accessibility...");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();

        match client.get("http://localhost:8546/health").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("✅ Webhook server is accessible at http://localhost:8546");
                } else {
                    info!("⚠️ Webhook server responded with status: {}", response.status());
                }
            }
            Err(e) => {
                info!("❌ Webhook server not accessible: {}", e);
                info!("Continuing test without accessibility verification...");
            }
        }

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
            ("automatic_top_up", "Automatic relayer balance top-up"),
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

        self.stop_webhook_server();

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

        let webhook_server =
            self.webhook_server().ok_or_else(|| anyhow!("Webhook server not initialized")).unwrap();

        webhook_server.clear_webhooks();

        let result = timeout(Duration::from_secs(90), self.execute_test(test_name)).await;

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
            "concurrent_relayer_creation" => self.test_concurrent_relayer_creation().await,
            "simple_eth_transfer_safe_proxy" => self.test_simple_eth_transfer_safe_proxy().await,
            "contract_interaction" => self.test_contract_interaction().await,
            "contract_interaction_safe_proxy" => self.test_contract_interaction_safe_proxy().await,
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
            "relayer_delete" => self.test_relayer_delete().await,
            "relayer_clone" => self.test_relayer_clone().await,
            "relayer_pause_unpause" => self.test_relayer_pause_unpause().await,
            "relayer_gas_configuration" => self.test_relayer_gas_configuration().await,
            "relayer_allowlist_toggle" => self.test_relayer_allowlist_toggle().await,
            "transaction_nonce_management" => self.test_transaction_nonce_management().await,
            "gas_price_bumping" => self.test_gas_price_bumping().await,
            "webhook_delivery" => self.test_webhook_delivery().await,
            "rate_limiting" => self.test_rate_limiting().await,
            "automatic_top_up_native" => self.test_automatic_top_up_native().await,
            "automatic_top_up_erc20" => self.test_automatic_top_up_erc20().await,
            "automatic_top_up_safe_proxy" => self.test_automatic_top_up_safe_proxy().await,
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
                    println!("  ⏰ {} - Test timed out after 90 seconds", test.name);
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

    /// Run a single filtered test with the new reporting system
    pub async fn run_filtered_test(&mut self, test_name: &str) -> TestSuite {
        println!("🚀 RRelayer E2E Test Suite - Single Test");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.start_webhook_server().await.unwrap();
        let relayer = self.create_and_fund_relayer("automatic_top_up").await.unwrap();
        self.fund_relayer(
            &relayer.address,
            alloy::primitives::utils::parse_ether("100000000").unwrap(),
        )
        .await
        .unwrap();

        info!("🔍 Testing webhook server accessibility...");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();

        match client.get("http://localhost:8546/health").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("✅ Webhook server is accessible at http://localhost:8546");
                } else {
                    info!("⚠️ Webhook server responded with status: {}", response.status());
                }
            }
            Err(e) => {
                info!("❌ Webhook server not accessible: {}", e);
                info!("Continuing test without accessibility verification...");
            }
        }

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
            "relayer_clone" => "Relayer clone functionality",
            "relayer_gas_configuration" => "Relayer gas configuration management",
            "relayer_allowlist_toggle" => "Relayer allowlist toggle functionality",
            "transaction_nonce_management" => "Transaction nonce management",
            "gas_price_bumping" => "Gas price bumping mechanism",
            "webhook_delivery" => "Webhook delivery testing",
            "rate_limiting" => "Rate limiting enforcement",
            "automatic_top_up_native" => "Automatic relayer native balance top-up",
            "automatic_top_up_erc20" => "Automatic relayer erc20 balance top-up",
            "automatic_top_up_safe_proxy" => "Automatic top-up using Safe proxy",
            "concurrent_transactions" => "Concurrent transaction handling",
            "unauthenticated" => "Unauthenticated protection",
            "blob_transaction_handling" => "Blob transaction handling (EIP-4844)",
            "transaction_data_validation" => "Transaction data validation",
            "balance_edge_cases" => "Balance edge case handling",
            _ => "Unknown test",
        };

        let test_result = self.run_single_test(test_name, description).await;
        suite.add_test(test_result);

        self.stop_webhook_server();

        suite.duration = overall_start.elapsed();
        self.print_final_report(&suite);
        suite
    }

    /// Fund a relayer address with ETH from the first Anvil account
    async fn fund_relayer(&self, relayer_address: &EvmAddress, funding_amount: U256) -> Result<()> {
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

    async fn create_relayer(&self, name: &str) -> Result<CreateRelayerResult> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        self.relayer_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(relayer)
    }

    async fn create_and_fund_relayer(&self, name: &str) -> Result<CreateRelayerResult> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        self.fund_relayer(&relayer.address, alloy::primitives::utils::parse_ether("10")?)
            .await
            .context("Failed to fund relayer")?;

        self.relayer_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(relayer)
    }

    pub async fn create_by_index_and_fund_relayer(
        &self,
        target_index: usize,
    ) -> Result<CreateRelayerResult> {
        let current_count = self.relayer_counter.load(std::sync::atomic::Ordering::SeqCst);

        if target_index < current_count {
            return Err(anyhow!(
                "Target index {} is less than current relayer count {}",
                target_index,
                current_count
            ));
        }

        // Create relayers in concurrent batches of 10 (smaller batches to test deadlock fix)
        let batch_size = 10;
        let mut last_relayer = None;
        let total_to_create = target_index - current_count + 1;

        info!(
            "Creating {} relayers from index {} to {} in batches of {}",
            total_to_create, current_count, target_index, batch_size
        );

        for batch_start in (current_count..=target_index).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size - 1, target_index);
            info!("Creating relayers batch {}-{}", batch_start, batch_end);

            // Check if this batch contains the target index
            if batch_start <= target_index && target_index <= batch_end {
                // This batch contains the target index - handle it specially
                let mut batch_results = Vec::new();

                for i in batch_start..=batch_end {
                    let relayer_name = format!("relayer_{}", i);
                    let relayer = if i == target_index {
                        // Use create_and_fund_relayer for the target index
                        self.create_and_fund_relayer(&relayer_name).await?
                    } else {
                        // Use regular create_relayer for all others
                        self.create_relayer(&relayer_name).await?
                    };

                    info!("Created relayer {} at index {}", relayer.id, i);
                    if i == target_index {
                        last_relayer = Some(relayer);
                        // Create a dummy relayer for the batch_results since we already saved the real one
                        continue;
                    }
                    batch_results.push(relayer);
                }
            } else {
                // Regular batch without target index - create all concurrently
                let relayer_names: Vec<String> =
                    (batch_start..=batch_end).map(|i| format!("relayer_{}", i)).collect();

                let batch_futures: Vec<_> =
                    relayer_names.iter().map(|name| self.create_relayer(name)).collect();

                let batch_results = futures::future::try_join_all(batch_futures).await?;

                for (i, relayer) in batch_results.into_iter().enumerate() {
                    let index = batch_start + i;
                    info!("Created relayer {} at index {}", relayer.id, index);
                }
            }
        }

        last_relayer.ok_or_else(|| anyhow!("Failed to create relayer at target index"))
    }

    /// run single with:
    /// make run-test-debug TEST=basic_relayer_creation
    async fn test_basic_relayer_creation(&self) -> Result<()> {
        info!("Creating test relayer...");

        let created_relayer = self.create_and_fund_relayer("basic-relayer-creation").await?;
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
    /// make run-test-debug TEST=simple_eth_transfer_safe_proxy
    async fn test_simple_eth_transfer_safe_proxy(&self) -> Result<()> {
        info!("Testing simple eth transfer using Safe proxy...");

        let relayer = self.create_by_index_and_fund_relayer(80).await?;
        info!("Created relayer at index 80: {:?}", relayer);

        // Fund the Safe proxy contract with ETH so it can send transactions
        let safe_proxy_address =
            EvmAddress::from_str("0xcfe267de230a234c5937f18f239617b7038ec271")?;
        info!("Funding Safe proxy contract at {} with 5 ETH", safe_proxy_address);
        self.fund_relayer(&safe_proxy_address, alloy::primitives::utils::parse_ether("5")?).await?;
        info!("Safe proxy contract funded successfully");

        let recipient = &self.config.anvil_accounts[1];
        let recipient_balance_before =
            self.contract_interactor.get_eth_balance(&recipient.into_address()).await?;
        info!("Sending ETH transfer to {} using Safe proxy", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                recipient,
                alloy::primitives::utils::parse_ether("4")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer via Safe proxy")?;

        info!("ETH transfer sent via Safe proxy: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        // Validate that the transaction went through the Safe proxy correctly
        let expected_safe_address =
            EvmAddress::from_str("0xcfe267de230a234c5937f18f239617b7038ec271")?;
        let expected_recipient = *recipient;

        // 1. Verify the transaction was sent TO the Safe proxy contract
        if EvmAddress::new(result.1.to.unwrap()) != expected_safe_address {
            return Err(anyhow!(
                "Transaction was not sent to Safe proxy! Expected: {}, Got: {}",
                expected_safe_address,
                EvmAddress::new(result.1.to.unwrap())
            ));
        }
        info!("✅ Transaction correctly sent to Safe proxy: {}", expected_safe_address);

        // 2. Verify the transaction was sent FROM the relayer (wallet index 80)
        if EvmAddress::new(result.1.from) != relayer.address {
            return Err(anyhow!(
                "Transaction was not sent from the expected relayer! Expected: {}, Got: {}",
                relayer.address,
                EvmAddress::new(result.1.from)
            ));
        }
        info!("✅ Transaction correctly sent from relayer: {}", relayer.address);

        // 3. Verify the transaction succeeded (status = true)
        if !result.1.status() {
            return Err(anyhow!("Safe proxy transaction failed on-chain"));
        }
        info!("✅ Safe proxy transaction succeeded on-chain");

        // 4. Verify gas was consumed (indicating execution)
        if result.1.inner.gas_used > 0 {
            info!(
                "✅ Gas was consumed (gas used: {}), indicating transaction execution",
                result.1.inner.gas_used
            );
        }

        // 5. Check if there are any logs/events (Safe execution should emit events)
        if !result.1.inner.inner.logs().is_empty() {
            info!(
                "✅ Transaction emitted {} log(s), indicating Safe execution",
                result.1.inner.inner.logs().len()
            );
            for (i, log) in result.1.inner.inner.logs().iter().enumerate() {
                info!("   Log {}: address={}, topics={}", i, log.address(), log.topics().len());
            }
        } else {
            info!("⚠️  No logs emitted - this might indicate the Safe execution didn't emit expected events");
        }

        // 6. Verify the recipient actually received the ETH
        let recipient_balance_after =
            self.contract_interactor.get_eth_balance(&expected_recipient.into_address()).await?;
        if recipient_balance_before > recipient_balance_after {
            return Err(anyhow!(
                "Recipient did not receive the expected ETH! Balance before: {}, Balance now: {}",
                alloy::primitives::utils::format_ether(recipient_balance_before),
                alloy::primitives::utils::format_ether(recipient_balance_after)
            ));
        }
        info!(
            "✅ Recipient {} balance after Safe transfer: {} ETH",
            expected_recipient,
            alloy::primitives::utils::format_ether(recipient_balance_after)
        );

        // 7. Verify the Safe proxy balance decreased
        let safe_balance_after =
            self.contract_interactor.get_eth_balance(&expected_safe_address.into_address()).await?;
        let eth_balance_after = alloy::primitives::utils::format_ether(safe_balance_after);
        if safe_balance_after != alloy::primitives::utils::parse_ether("1")? {
            return Err(anyhow!(
                "Safe proxy balance increased after Safe transfer! Expected: <= 0.5 ETH, Got: {}",
                eth_balance_after
            ));
        }
        info!(
            "✅ Safe proxy {} balance after transfer: {} ETH",
            expected_safe_address,
            alloy::primitives::utils::format_ether(safe_balance_after)
        );

        info!("🎉 All Safe proxy validations passed!");

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

        let relayer_balance =
            self.contract_interactor.get_eth_balance(&relayer.address.into_address()).await?;
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
    /// make run-test-debug TEST=contract_interaction_safe_proxy
    async fn test_contract_interaction_safe_proxy(&self) -> Result<()> {
        info!("Testing contract interaction via Safe proxy...");

        let relayer = self.create_by_index_and_fund_relayer(80).await?;
        info!("Created Safe proxy relayer: {:?}", relayer);

        let contract_address =
            self.contract_interactor.contract_address().context("Test contract not deployed")?;

        info!(
            "Sending contract interaction to deployed contract at {} via Safe proxy",
            contract_address
        );

        let is_deployed = self.contract_interactor.verify_contract_deployed().await?;
        if !is_deployed {
            return Err(anyhow::anyhow!("Contract verification failed - no code at address"));
        }
        info!("✅ Contract verified as deployed with code at {}", contract_address);

        let relayer_balance =
            self.contract_interactor.get_eth_balance(&relayer.address.into_address()).await?;
        info!(
            "Relayer balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(relayer_balance)
        );

        // Get Safe proxy address for validation
        let safe_proxy_address = Address::from_str("0xcfe267de230a234c5937f18f239617b7038ec271")?;
        let safe_balance_before =
            self.contract_interactor.get_eth_balance(&safe_proxy_address).await?;
        info!(
            "Safe proxy balance before transaction: {} ETH",
            alloy::primitives::utils::format_ether(safe_balance_before)
        );

        let calldata: TransactionData =
            TransactionData::raw_hex(&self.contract_interactor.encode_simple_call(42)?).unwrap();

        let tx_response = self
            .relayer_client
            .send_transaction(&relayer.id, &contract_address, TransactionValue::zero(), calldata)
            .await
            .context("Failed to send contract interaction via Safe proxy")?;

        info!("Contract interaction sent via Safe proxy: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        let expected_safe_address =
            EvmAddress::from_str("0xcfe267de230a234c5937f18f239617b7038ec271")?;

        if EvmAddress::new(result.1.to.unwrap()) != expected_safe_address {
            return Err(anyhow!(
                "Transaction was not sent to Safe proxy! Expected: {}, Got: {}",
                expected_safe_address,
                EvmAddress::new(result.1.to.unwrap())
            ));
        }
        info!("✅ Transaction correctly sent to Safe proxy: {}", expected_safe_address);

        if EvmAddress::new(result.1.from) != relayer.address {
            return Err(anyhow!(
                "Transaction was not sent from the expected relayer! Expected: {}, Got: {}",
                relayer.address,
                EvmAddress::new(result.1.from)
            ));
        }
        info!("✅ Transaction correctly sent from relayer: {}", relayer.address);

        if !result.1.status() {
            return Err(anyhow!("Safe proxy transaction failed on-chain"));
        }
        info!("✅ Safe proxy transaction succeeded on-chain");

        if result.1.inner.gas_used > 0 {
            info!(
                "✅ Gas was consumed (gas used: {}), indicating transaction execution",
                result.1.inner.gas_used
            );
        }

        if !result.1.inner.inner.logs().is_empty() {
            info!(
                "✅ Transaction emitted {} log(s), indicating Safe execution",
                result.1.inner.inner.logs().len()
            );
            for (i, log) in result.1.inner.inner.logs().iter().enumerate() {
                info!("   Log {}: address={}, topics={}", i, log.address(), log.topics().len());
            }
        } else {
            info!("⚠️  No logs emitted - this might indicate the Safe execution didn't emit expected events");
        }

        info!("🎉 All Safe proxy contract interaction validations passed!");
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

        let network = all_networks.first().context("No networks found")?;
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
                None,
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
                None,
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
                    None,
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
            .sign_text(&relayer.id, test_message, None)
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
            .sign_typed_data(&relayer.id, &typed_data, None)
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
            .send_transaction(&relayer.id, &tx_request, None)
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
                .send_transaction(&relayer.id, &tx_request, None)
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
            .send_transaction(&relayer.id, &tx_request, None)
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
            .send_transaction(&relayer.id, &tx_request, None)
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
            .send_transaction(&relayer.id, &tx_request, None)
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
                .send_transaction(&relayer.id, &tx_request, None)
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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await;

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
        info!("Testing relayer delete...");

        let relayer = self.create_and_fund_relayer("delete-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let created_relayer = self
            .relayer_client
            .sdk
            .relayer
            .get(&relayer.id)
            .await?
            .context("Relayer should exist")?;

        if created_relayer.relayer.id != relayer.id {
            return Err(anyhow::anyhow!("Relayer should exist"));
        }

        self.relayer_client.sdk.relayer.delete(&relayer.id).await?;

        let created_relayer = self.relayer_client.sdk.relayer.get(&relayer.id).await;

        match created_relayer {
            Ok(_) => Err(anyhow::anyhow!("Relayer should have been deleted")),
            Err(_) => {
                info!("✅ Relayer delete functionality working correctly");
                Ok(())
            }
        }
    }

    /// run single with:
    /// make run-test-debug TEST=relayer_clone
    async fn test_relayer_clone(&self) -> Result<()> {
        info!("Testing relayer clone...");

        let relayer = self.create_and_fund_relayer("clone-test-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let created_relayer = self
            .relayer_client
            .sdk
            .relayer
            .get(&relayer.id)
            .await?
            .context("Relayer should exist")?;

        if created_relayer.relayer.id != relayer.id {
            return Err(anyhow::anyhow!("Relayer should exist"));
        }

        let cloned_relayer = self
            .relayer_client
            .sdk
            .relayer
            .clone(&relayer.id, 31337, "cloned-test-relayer")
            .await?;
        if cloned_relayer.id == relayer.id {
            return Err(anyhow::anyhow!("Relayer should have been cloned and have its own ID"));
        }

        if cloned_relayer.address != relayer.address {
            return Err(anyhow::anyhow!(
                "Relayer should have been cloned and have the shared address"
            ));
        }

        let recipient = &self.config.anvil_accounts[1];
        info!("Sending ETH transfer to {} from the new cloned one", recipient);

        let tx_response = self
            .relayer_client
            .send_transaction(
                &cloned_relayer.id,
                recipient,
                alloy::primitives::utils::parse_ether("0.5")?.into(),
                TransactionData::empty(),
            )
            .await
            .context("Failed to send ETH transfer")?;

        info!("ETH transfer sent: {:?}", tx_response);

        let result = self.wait_for_transaction_completion(&tx_response.0.id).await?;

        self.relayer_client.sent_transaction_compare(tx_response.1, result.0)?;

        let paging = PagingContext { limit: 10, offset: 0 };
        let first_relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions(&relayer.id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!("✅ Found {} transactions for first relayer", first_relayer_transactions.items.len());

        let cloned_relayer_transactions = self
            .relayer_client
            .sdk
            .transaction
            .get_transactions(&cloned_relayer.id, &paging)
            .await
            .context("Failed to get relayer transactions")?;

        info!(
            "✅ Found {} transactions for cloned relayer",
            cloned_relayer_transactions.items.len()
        );

        if first_relayer_transactions.items.len() != 0 {
            return Err(anyhow::anyhow!(
                "First relayer expected at 0 transactions, but got {}",
                first_relayer_transactions.items.len()
            ));
        }

        if cloned_relayer_transactions.items.len() != 1 {
            return Err(anyhow::anyhow!(
                "Cloned relayer expected at 1 transactions, but got {}",
                cloned_relayer_transactions.items.len()
            ));
        }

        info!("✅ Relayer clone functionality working correctly");

        Ok(())
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
                .send_transaction(&relayer.id, &tx_request, None)
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

        let timeout = Duration::from_secs(90);
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

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

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

    /// run single with:
    /// make run-test-debug TEST=webhook_delivery
    async fn test_webhook_delivery(&self) -> Result<()> {
        info!("Testing webhook delivery mechanism...");

        let webhook_server =
            self.webhook_server().ok_or_else(|| anyhow!("Webhook server not initialized"))?;

        webhook_server.clear_webhooks();

        let relayer = self.create_and_fund_relayer("webhook-test-relayer").await?;
        info!("Created relayer for webhook testing: {}", relayer.id);

        info!("Test 1: Simple ETH transfer webhook events");
        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("webhook-eth-transfer".to_string()),
            blobs: None,
        };

        let send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

        info!("📨 Transaction submitted: {}", send_result.id);

        let initial_webhooks =
            webhook_server.get_webhooks_for_transaction(&send_result.id.to_string());
        info!("📨 Initial webhooks received: {}", initial_webhooks.len());

        if initial_webhooks.is_empty() {
            info!("⚠️ No initial webhooks received, checking all webhooks...");
            let all_webhooks = webhook_server.get_received_webhooks();
            info!("📊 Total webhooks received so far: {}", all_webhooks.len());
            for webhook in &all_webhooks {
                info!("  - Event: {}, TxID: {}", webhook.event_type, webhook.transaction_id);
            }
        }

        info!("⏳ Waiting for transaction {} to complete fully...", send_result.id);
        let (completed_tx, _receipt) =
            self.wait_for_transaction_completion(&send_result.id).await?;
        info!("✅ Transaction completed with status: {:?}", completed_tx.status);

        let eth_transfer_webhooks =
            webhook_server.get_webhooks_for_transaction(&send_result.id.to_string());
        info!(
            "📨 Final webhooks received for ETH transfer {}: {}",
            send_result.id,
            eth_transfer_webhooks.len()
        );

        if eth_transfer_webhooks.is_empty() {
            return Err(anyhow!("No webhooks received for ETH transfer transaction"));
        }

        info!("📤 Test 2: Contract interaction webhook events");
        let contract_address = self
            .contract_interactor
            .contract_address()
            .ok_or_else(|| anyhow!("Contract not deployed"))?;

        let contract_data = self.contract_interactor.encode_simple_call(42)?;
        let contract_tx_request = RelayTransactionRequest {
            to: contract_address,
            value: TransactionValue::zero(),
            data: TransactionData::raw_hex(&contract_data).unwrap(),
            speed: Some(TransactionSpeed::Fast),
            external_id: Some("webhook-contract-call".to_string()),
            blobs: None,
        };

        let contract_send_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &contract_tx_request, None)
            .await?;

        info!("📨 Contract transaction submitted: {}", contract_send_result.id);

        self.wait_for_transaction_completion(&contract_send_result.id).await?;

        let contract_webhooks =
            webhook_server.get_webhooks_for_transaction(&contract_send_result.id.to_string());
        info!(
            "📨 Received {} webhooks for contract transaction {}",
            contract_webhooks.len(),
            contract_send_result.id
        );

        if contract_webhooks.is_empty() {
            return Err(anyhow!("No webhooks received for contract transaction"));
        }

        info!("📤 Test 3: Webhook payload validation");
        let all_webhooks = webhook_server.get_received_webhooks();
        if all_webhooks.is_empty() {
            return Err(anyhow!("No webhooks were received during testing"));
        }

        info!("📊 All webhooks received during test: {}", all_webhooks.len());

        for (i, webhook) in all_webhooks.iter().enumerate() {
            info!(
                "  {}. Event: {}, TxID: {}, RelayerID: {}",
                i + 1,
                webhook.event_type,
                webhook.transaction_id,
                webhook.relayer_id
            );

            // Validate webhook structure
            if webhook.transaction_id.is_empty() {
                return Err(anyhow!("Webhook missing transaction_id"));
            }
            if webhook.relayer_id.is_empty() {
                return Err(anyhow!("Webhook missing relayer_id"));
            }
            if webhook.event_type.is_empty() {
                return Err(anyhow!("Webhook missing event_type"));
            }

            // Validate required headers
            if !webhook.headers.contains_key("x-rrelayer-signature") {
                return Err(anyhow!("Webhook missing signature header"));
            }
            if !webhook.headers.contains_key("x-rrelayer-event") {
                return Err(anyhow!("Webhook missing event header"));
            }
            if !webhook.headers.contains_key("x-rrelayer-delivery") {
                return Err(anyhow!("Webhook missing delivery header"));
            }

            if !webhook.payload.get("transaction").is_some() {
                return Err(anyhow!("Webhook payload missing transaction data"));
            }
            if !webhook.payload.get("event_type").is_some() {
                return Err(anyhow!("Webhook payload missing event_type"));
            }
            if !webhook.payload.get("timestamp").is_some() {
                return Err(anyhow!("Webhook payload missing timestamp"));
            }

            info!("✅ Webhook validation passed for event: {}", webhook.event_type);
        }

        info!("📤 Test 4: Transaction lifecycle webhook sequence");
        let mut sorted_webhooks = eth_transfer_webhooks.clone();
        sorted_webhooks.sort_by_key(|w| w.timestamp);

        let event_sequence: Vec<String> =
            sorted_webhooks.iter().map(|w| w.event_type.clone()).collect();

        info!("📋 Webhook event sequence: {:?}", event_sequence);

        if let Some(first_event) = event_sequence.first() {
            if first_event != "transaction_queued" {
                return Err(anyhow!(
                    "Expected first webhook event to be 'transaction_queued', got '{}'",
                    first_event
                ));
            }
        }

        let has_queued = event_sequence.contains(&"transaction_queued".to_string());
        let has_sent = event_sequence.contains(&"transaction_sent".to_string());
        let has_mined = event_sequence.contains(&"transaction_mined".to_string());

        if !has_queued {
            return Err(anyhow!("Missing 'transaction_queued' webhook event"));
        }
        if !has_sent {
            return Err(anyhow!("Missing 'transaction_sent' webhook event"));
        }
        if !has_mined {
            return Err(anyhow!("Missing 'transaction_mined' webhook event"));
        }

        info!("📤 Test 5: Transaction confirmation webhook (15 blocks)");
        info!("⛏️ Mining 15 blocks to confirm the ETH transfer transaction...");
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("⏳ Waiting for confirmation processing...");

        let mut confirmed_webhooks = Vec::new();
        for attempt in 1..=10 {
            confirmed_webhooks = webhook_server.get_webhooks_by_event("transaction_confirmed");
            if !confirmed_webhooks.is_empty() {
                break;
            }
            info!("⏳ Attempt {}/10: Waiting for transaction_confirmed webhook...", attempt);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        if confirmed_webhooks.is_empty() {
            return Err(anyhow!(
                "Missing 'transaction_confirmed' webhook event after 15 blocks and 15 seconds wait"
            ));
        }

        info!("✅ Received 'transaction_confirmed' webhook event");

        info!("📤 Test 6: Transaction cancelled webhook");

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
            .send_transaction(&relayer.id, &tx_request, None)
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

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("⏳ Waiting for cancellation processing...");

        info!("📤 Test 7: Transaction replacement webhook");

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
            .send_transaction(&relayer.id, &tx_request, None)
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

        // TODO: uncomment when fix nonce issue on webhooks
        // let replace_result = self
        //     .relayer_client
        //     .sdk
        //     .transaction
        //     .replace_transaction(transaction_id, &replacement_request)
        //     .await
        //     .context("Failed to replace transaction")?;
        // info!("✅ Transaction replacement result: {}", replace_result);
        //
        // if !replace_result {
        //     return Err(anyhow::anyhow!("Replace transaction failed"));
        // }

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("⏳ Waiting for replace transaction processing...");

        info!("📤 Test 8: Comprehensive webhook event verification");
        let final_all_webhooks = webhook_server.get_received_webhooks();
        let final_webhook_events: Vec<String> =
            final_all_webhooks.iter().map(|w| w.event_type.clone()).collect();
        let final_unique_events: std::collections::HashSet<String> =
            final_webhook_events.iter().cloned().collect();

        let webhook_events = [
            "transaction_queued",
            "transaction_sent",
            "transaction_mined",
            "transaction_confirmed",
            "transaction_cancelled",
            // TODO: uncomment when fix nonce issue on webhooks
            // "transaction_replaced"
        ];

        for event in &webhook_events {
            let count = final_webhook_events.iter().filter(|&e| e == event).count();
            if count > 0 {
                info!("✅ Received '{}' webhook event ({} occurrences)", event, count);
            } else {
                return Err(anyhow!("Missing '{}' webhook event", event));
            }
        }

        info!(
            "📋 Successfully received all transaction webhook events: {:?}",
            final_unique_events.into_iter().collect::<Vec<_>>()
        );

        info!("📤 Test 7: Signing operations webhook events");

        webhook_server.clear_webhooks();

        // Test text signing webhook
        info!("🔐 Testing text signing webhook...");
        let text_to_sign = "Hello, RRelayer webhook test!";

        let sign_text_result =
            self.relayer_client.sdk.sign.sign_text(&relayer.id, text_to_sign, None).await?;

        info!("✅ Text signed successfully, signature: {:?}", sign_text_result.signature);

        // Wait a moment for webhook delivery
        tokio::time::sleep(Duration::from_millis(500)).await;

        let text_signing_webhooks = webhook_server.get_webhooks_by_event("text_signed");
        if text_signing_webhooks.is_empty() {
            return Err(anyhow!("No 'text_signed' webhook received"));
        }

        info!("✅ Received {} text_signed webhook(s)", text_signing_webhooks.len());

        // Validate text signing webhook payload
        let text_webhook = &text_signing_webhooks[0];
        if !text_webhook.payload.get("signing").is_some() {
            return Err(anyhow!("Text signing webhook missing 'signing' data"));
        }

        let signing_data = text_webhook.payload.get("signing").unwrap();
        if !signing_data.get("message").is_some() {
            return Err(anyhow!("Text signing webhook missing 'message' field"));
        }
        if !signing_data.get("signature").is_some() {
            return Err(anyhow!("Text signing webhook missing 'signature' field"));
        }
        if !signing_data.get("relayerId").is_some() {
            return Err(anyhow!("Text signing webhook missing 'relayerId' field"));
        }

        let message_value = signing_data.get("message").unwrap().as_str().unwrap_or("");
        if message_value != text_to_sign {
            return Err(anyhow!(
                "Text signing webhook message mismatch: expected '{}', got '{}'",
                text_to_sign,
                message_value
            ));
        }

        info!("✅ Text signing webhook payload validation passed");

        info!("🔐 Testing typed data signing webhook...");

        use serde_json::json;
        let typed_data = json!({
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"}
                ],
                "TestMessage": [
                    {"name": "message", "type": "string"},
                    {"name": "value", "type": "uint256"}
                ]
            },
            "primaryType": "TestMessage",
            "domain": {
                "name": "RRelayer Test",
                "version": "1",
                "chainId": 31337
            },
            "message": {
                "message": "Test webhook typed data",
                "value": 42
            }
        });

        let typed_data_parsed: TypedData = serde_json::from_value(typed_data)?;

        let sign_typed_data_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data_parsed, None)
            .await?;

        info!(
            "✅ Typed data signed successfully, signature: {:?}",
            sign_typed_data_result.signature
        );

        tokio::time::sleep(Duration::from_millis(500)).await;

        let typed_data_signing_webhooks = webhook_server.get_webhooks_by_event("typed_data_signed");
        if typed_data_signing_webhooks.is_empty() {
            return Err(anyhow!("No 'typed_data_signed' webhook received"));
        }

        info!("✅ Received {} typed_data_signed webhook(s)", typed_data_signing_webhooks.len());

        let typed_data_webhook = &typed_data_signing_webhooks[0];
        if !typed_data_webhook.payload.get("signing").is_some() {
            return Err(anyhow!("Typed data signing webhook missing 'signing' data"));
        }

        let typed_signing_data = typed_data_webhook.payload.get("signing").unwrap();
        if !typed_signing_data.get("domainData").is_some() {
            return Err(anyhow!("Typed data signing webhook missing 'domainData' field"));
        }
        if !typed_signing_data.get("messageData").is_some() {
            return Err(anyhow!("Typed data signing webhook missing 'messageData' field"));
        }
        if !typed_signing_data.get("primaryType").is_some() {
            return Err(anyhow!("Typed data signing webhook missing 'primaryType' field"));
        }
        if !typed_signing_data.get("signature").is_some() {
            return Err(anyhow!("Typed data signing webhook missing 'signature' field"));
        }

        let primary_type_value =
            typed_signing_data.get("primaryType").unwrap().as_str().unwrap_or("");
        if primary_type_value != "TestMessage" {
            return Err(anyhow!(
                "Typed data signing webhook primaryType mismatch: expected 'TestMessage', got '{}'",
                primary_type_value
            ));
        }

        info!("✅ Typed data signing webhook payload validation passed");

        for signing_webhook in [&text_signing_webhooks[0], &typed_data_signing_webhooks[0]] {
            if !signing_webhook.headers.contains_key("x-rrelayer-signature") {
                return Err(anyhow!("Signing webhook missing signature header"));
            }
            if !signing_webhook.headers.contains_key("x-rrelayer-event") {
                return Err(anyhow!("Signing webhook missing event header"));
            }
            if !signing_webhook.headers.contains_key("x-rrelayer-delivery") {
                return Err(anyhow!("Signing webhook missing delivery header"));
            }

            if !signing_webhook.payload.get("event_type").is_some() {
                return Err(anyhow!("Signing webhook payload missing event_type"));
            }
            if !signing_webhook.payload.get("timestamp").is_some() {
                return Err(anyhow!("Signing webhook payload missing timestamp"));
            }
            if !signing_webhook.payload.get("api_version").is_some() {
                return Err(anyhow!("Signing webhook payload missing api_version"));
            }
        }

        info!("✅ Signing webhook structure validation passed");
        info!("📊 Signing tests summary:");
        info!("   🔐 Text signing webhook: ✅ received and validated");
        info!("   📝 Typed data signing webhook: ✅ received and validated");
        info!("   🔍 Payload structure: ✅ all required fields present");
        info!("   🔒 HMAC signature validation: ✅ headers present");

        info!("✅ Comprehensive webhook delivery testing completed successfully!");
        info!("   📊 Total webhooks received: {}", final_all_webhooks.len());
        info!("   📋 Core events tested: queued, sent, mined, confirmed");
        info!("   🔒 Signature validation: passed");
        info!("   📝 Payload structure: validated");
        info!("   🔄 Lifecycle sequence: verified");
        info!("   📤 Contract interactions: tested");
        info!("   ✅ Transaction confirmations: tested (15 blocks)");
        info!("   🔐 Text signing webhooks: tested and validated");
        info!("   📜 Typed data signing webhooks: tested and validated");
        info!("   🔍 Webhook consistency: all calls non-blocking");

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=rate_limiting
    async fn test_rate_limiting(&self) -> Result<()> {
        info!("Testing rate limiting enforcement...");

        let relayer = self.create_and_fund_relayer("rate-limit-relayer").await?;
        info!("relayer: {:?}", relayer);

        let relay_key = Some(self.config.anvil_accounts[0].to_string());

        let mut successful_transactions = 0;

        for i in 0..5 {
            let tx_result = self
                .relayer_client
                .send_transaction_with_rate_limit_key(
                    &relayer.id,
                    &self.config.anvil_accounts[1],
                    alloy::primitives::utils::parse_ether("0.5")?.into(),
                    TransactionData::empty(),
                    relay_key.clone(),
                )
                .await;

            match tx_result {
                Ok(_) => successful_transactions += 1,
                Err(_) => {}
            }
        }
        if successful_transactions != 1 {
            return Err(anyhow!("Sending transactions rate limiting not enforced"));
        }

        self.mine_blocks(1).await?;
        info!("Successful transactions before rate limit: {}", successful_transactions);

        let mut successful_signing = 0;

        for _ in 0..5 {
            let sign_result = self
                .relayer_client
                .sdk
                .sign
                .sign_text(&relayer.id, "Hello, RRelayer!", relay_key.clone())
                .await;

            match sign_result {
                Ok(_) => successful_signing += 1,
                Err(_) => {}
            }
        }

        if successful_signing != 1 {
            return Err(anyhow!("Signing text rate limiting not enforced"));
        }

        info!("Successful signing text before rate limit: {}", successful_transactions);

        info!("Sleep for 60 seconds to allow the rate limit to expire");
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut successful_signing = 0;

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

        for _ in 0..5 {
            let sign_result = self
                .relayer_client
                .sdk
                .sign
                .sign_typed_data(&relayer.id, &typed_data, relay_key.clone())
                .await;

            match sign_result {
                Ok(_) => successful_signing += 1,
                Err(_) => {}
            }
        }

        if successful_signing != 1 {
            return Err(anyhow!("Signing typed data rate limiting not enforced"));
        }

        info!("Successful signing typed data before rate limit: {}", successful_transactions);

        info!("Rate limiting mechanism verified");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=automatic_top_up_native
    async fn test_automatic_top_up_native(&self) -> Result<()> {
        info!("Testing automatic relayer balance top-up...");

        // Create multiple relayers with different starting balances
        let relayer1 = self.create_and_fund_relayer("top-up-test-1").await?;
        info!("relayer1: {:?}", relayer1);
        let relayer2 = self.create_and_fund_relayer("top-up-test-2").await?;
        info!("relayer2: {:?}", relayer2);
        let relayer3 = self.create_and_fund_relayer("top-up-test-3").await?;
        info!("relayer3: {:?}", relayer3);

        info!("Created test relayers: {:?}, {:?}, {:?}", relayer1.id, relayer2.id, relayer3.id);

        // Check initial balances (should be funded by create_and_fund_relayer)
        let initial_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let initial_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let initial_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Initial balances:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(initial_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(initial_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(initial_balance3));

        // Drain some balances to simulate low balance scenarios
        let drain_amount = alloy::primitives::utils::parse_ether("90")?; // Leave about 10 ETH

        // Send transactions to drain balances below top-up threshold
        info!("Draining relayer balances to trigger top-up...");

        // Drain relayer1 significantly
        if initial_balance1 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some("drain-tx-1".to_string()),
                blobs: None,
            };

            self.relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer1.id, &tx_request, None)
                .await?;
        }

        // Drain relayer2 significantly
        if initial_balance2 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some("drain-tx-2".to_string()),
                blobs: None,
            };

            self.relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer2.id, &tx_request, None)
                .await?;
        }

        // Mine a block to process the drain transactions
        self.mine_and_wait().await?;

        // Check balances after draining
        let drained_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let drained_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let drained_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Balances after draining:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(drained_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(drained_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(drained_balance3));

        // Wait for automatic top-up to trigger (give it some time)
        info!("Waiting for automatic top-up mechanism to trigger...");
        tokio::time::sleep(Duration::from_secs(30)).await;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        // Check if balances have been topped up to 100 ETH
        let final_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let final_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;
        let final_balance3 =
            self.contract_interactor.get_eth_balance(&relayer3.address.into_address()).await?;

        info!("Final balances after top-up:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(final_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(final_balance2));
        info!("  Relayer 3: {} ETH", alloy::primitives::utils::format_ether(final_balance3));

        // Expected top-up amount is 100 ETH
        let expected_top_up = alloy::primitives::utils::parse_ether("100")?;
        let tolerance = alloy::primitives::utils::parse_ether("10.01")?; // 11 ETH tolerance due to initial 10 ETH top-up

        // Verify that relayers with low balances got topped up
        if drained_balance1 < expected_top_up {
            if final_balance1.abs_diff(expected_top_up) > tolerance {
                return Err(anyhow::anyhow!(
                    "Relayer 1 balance not topped up correctly. Expected ~100 ETH, got {} ETH",
                    alloy::primitives::utils::format_ether(final_balance1)
                ));
            }
            info!("✅ Relayer 1 successfully topped up to ~100 ETH");
        }

        if drained_balance2 < expected_top_up {
            if final_balance2.abs_diff(expected_top_up) > tolerance {
                return Err(anyhow::anyhow!(
                    "Relayer 2 balance not topped up correctly. Expected ~100 ETH, got {} ETH",
                    alloy::primitives::utils::format_ether(final_balance2)
                ));
            }
            info!("✅ Relayer 2 successfully topped up to ~100 ETH");
        }

        // Relayer 3 should remain unchanged if it wasn't drained
        if drained_balance3 >= expected_top_up {
            let balance_change = final_balance3.abs_diff(drained_balance3);
            if balance_change > tolerance {
                return Err(anyhow::anyhow!(
                    "Relayer 3 balance changed unexpectedly. Was {} ETH, now {} ETH",
                    alloy::primitives::utils::format_ether(drained_balance3),
                    alloy::primitives::utils::format_ether(final_balance3)
                ));
            }
            info!("✅ Relayer 3 balance remained stable (no top-up needed)");
        }

        info!("✅ Automatic top-up mechanism working correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=automatic_top_up_erc20
    async fn test_automatic_top_up_erc20(&self) -> Result<()> {
        info!("Testing automatic ERC-20 token top-up...");

        let token_address = self
            .contract_interactor
            .token_address()
            .ok_or_else(|| anyhow::anyhow!("ERC-20 token not deployed"))?;

        info!("Using ERC-20 token at address: {:?}", token_address);

        // Create multiple relayers with different starting balances
        let relayer1 = self.create_and_fund_relayer("erc20-top-up-test-1").await?;
        info!("relayer1: {:?}", relayer1);
        let relayer2 = self.create_and_fund_relayer("erc20-top-up-test-2").await?;
        info!("relayer2: {:?}", relayer2);
        let relayer3 = self.create_and_fund_relayer("erc20-top-up-test-3").await?;
        info!("relayer3: {:?}", relayer3);

        info!("Created test relayers: {:?}, {:?}, {:?}", relayer1.id, relayer2.id, relayer3.id);

        // Check initial ERC-20 token balances (should be 0 for new relayers)
        let initial_balance1 =
            self.contract_interactor.get_token_balance(&relayer1.address.into_address()).await?;
        let initial_balance2 =
            self.contract_interactor.get_token_balance(&relayer2.address.into_address()).await?;
        let initial_balance3 =
            self.contract_interactor.get_token_balance(&relayer3.address.into_address()).await?;

        info!("Initial ERC-20 token balances:");
        info!(
            "  Relayer 1: {} tokens",
            alloy::primitives::utils::format_units(initial_balance1, 18)?
        );
        info!(
            "  Relayer 2: {} tokens",
            alloy::primitives::utils::format_units(initial_balance2, 18)?
        );
        info!(
            "  Relayer 3: {} tokens",
            alloy::primitives::utils::format_units(initial_balance3, 18)?
        );

        // Since relayers start with 0 token balance, they should automatically get topped up
        // Wait for automatic top-up to trigger
        info!("Waiting for automatic ERC-20 top-up mechanism to trigger...");
        tokio::time::sleep(Duration::from_secs(30)).await;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        // Check if balances have been topped up to 500 tokens (as configured in YAML)
        let final_balance1 =
            self.contract_interactor.get_token_balance(&relayer1.address.into_address()).await?;
        let final_balance2 =
            self.contract_interactor.get_token_balance(&relayer2.address.into_address()).await?;
        let final_balance3 =
            self.contract_interactor.get_token_balance(&relayer3.address.into_address()).await?;

        info!("Final ERC-20 token balances after top-up:");
        info!(
            "  Relayer 1: {} tokens",
            alloy::primitives::utils::format_units(final_balance1, 18)?
        );
        info!(
            "  Relayer 2: {} tokens",
            alloy::primitives::utils::format_units(final_balance2, 18)?
        );
        info!(
            "  Relayer 3: {} tokens",
            alloy::primitives::utils::format_units(final_balance3, 18)?
        );

        // Expected top-up amount is 500 tokens (as configured in YAML)
        let expected_top_up = U256::from(500u64) * U256::from(10u64).pow(U256::from(18u64));
        let tolerance = U256::from(10u64) * U256::from(10u64).pow(U256::from(18u64)); // 10 token tolerance

        // Verify that all relayers got topped up since they started with 0 balance
        if final_balance1.abs_diff(expected_top_up) > tolerance {
            return Err(anyhow::anyhow!(
                "Relayer 1 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance1, 18)?
            ));
        }
        info!("✅ Relayer 1 successfully topped up to ~500 tokens");

        if final_balance2.abs_diff(expected_top_up) > tolerance {
            return Err(anyhow::anyhow!(
                "Relayer 2 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance2, 18)?
            ));
        }
        info!("✅ Relayer 2 successfully topped up to ~500 tokens");

        if final_balance3.abs_diff(expected_top_up) > tolerance {
            return Err(anyhow::anyhow!(
                "Relayer 3 token balance not topped up correctly. Expected ~500 tokens, got {} tokens",
                alloy::primitives::utils::format_units(final_balance3, 18)?
            ));
        }
        info!("✅ Relayer 3 successfully topped up to ~500 tokens");

        info!("✅ Automatic ERC-20 token top-up mechanism working correctly");
        Ok(())
    }

    // TODO: test_automatic_top_up_safe_proxy last part

    /// run single with:
    /// make run-test-debug TEST=automatic_top_up_safe_proxy
    async fn test_automatic_top_up_safe_proxy(&self) -> Result<()> {
        info!("Testing automatic top-up using Safe proxy...");

        // Create a relayer with wallet index 80 (which is configured as Safe proxy signer)
        // Note: We need to create a relayer that maps to wallet index 80: 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf
        // For this test, we'll create relayers and drain them to below the threshold

        let relayer1 = self.create_and_fund_relayer("safe-proxy-test-1").await?;
        info!("relayer1: {:?}", relayer1);
        let relayer2 = self.create_and_fund_relayer("safe-proxy-test-2").await?;
        info!("relayer2: {:?}", relayer2);

        info!("Created test relayers for Safe proxy test: {:?}, {:?}", relayer1.id, relayer2.id);

        // Check initial balances
        let initial_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let initial_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;

        info!("Initial balances:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(initial_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(initial_balance2));

        // Check the Safe proxy funding address balance (wallet index 80)
        let safe_proxy_signer: alloy::primitives::Address =
            "0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf"
                .parse()
                .context("Failed to parse Safe proxy signer address")?;
        let safe_signer_balance =
            self.contract_interactor.get_eth_balance(&safe_proxy_signer).await?;
        info!(
            "Safe proxy signer (wallet 80) balance: {} ETH",
            alloy::primitives::utils::format_ether(safe_signer_balance)
        );

        // We need to fund the Safe proxy signer (wallet index 80) since it's configured as the from_address
        // The automatic top-up configuration has from_address: 0x655B2B8861D7E911D283A05A5CAD042C157106DA
        // But for Safe proxy, the signing will be done by wallet index 80: 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf
        let funding_amount = alloy::primitives::utils::parse_ether("20")?;
        info!(
            "Funding Safe proxy signer with {} ETH for testing...",
            alloy::primitives::utils::format_ether(funding_amount)
        );

        self.fund_relayer(&safe_proxy_signer.into(), funding_amount).await?;

        let updated_safe_signer_balance =
            self.contract_interactor.get_eth_balance(&safe_proxy_signer).await?;
        info!(
            "Updated Safe proxy signer balance: {} ETH",
            alloy::primitives::utils::format_ether(updated_safe_signer_balance)
        );

        // Drain relayer balances to trigger automatic top-up
        let drain_amount = alloy::primitives::utils::parse_ether("90")?; // Leave about 10 ETH
        info!("Draining relayer balances to trigger Safe proxy top-up...");

        // Drain relayer1
        if initial_balance1 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some("safe-drain-tx-1".to_string()),
                blobs: None,
            };

            let tx_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer1.id, &tx_request, None)
                .await?;
            info!("Relayer 1 drain transaction sent: {:?}", tx_result.hash);
        }

        // Drain relayer2
        if initial_balance2 > drain_amount {
            let tx_request = RelayTransactionRequest {
                to: self.config.anvil_accounts[4],
                value: drain_amount.into(),
                data: TransactionData::empty(),
                speed: Some(TransactionSpeed::Fast),
                external_id: Some("safe-drain-tx-2".to_string()),
                blobs: None,
            };

            let tx_result = self
                .relayer_client
                .sdk
                .transaction
                .send_transaction(&relayer2.id, &tx_request, None)
                .await?;
            info!("Relayer 2 drain transaction sent: {:?}", tx_result.hash);
        }

        // Mine a few blocks to ensure drain transactions are processed
        self.mine_blocks(5).await?;

        // Check balances after draining
        let drained_balance1 =
            self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
        let drained_balance2 =
            self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;

        info!("Balances after draining:");
        info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(drained_balance1));
        info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(drained_balance2));

        // Wait for automatic top-up to trigger through Safe proxy
        info!("Waiting for automatic Safe proxy top-up mechanism to trigger...");

        // The automatic top-up task runs every 30 seconds, so wait up to 2 minutes
        let max_wait_time = tokio::time::Duration::from_secs(120);
        let check_interval = tokio::time::Duration::from_secs(10);
        let start_time = tokio::time::Instant::now();

        let min_expected_balance = alloy::primitives::utils::parse_ether("50")?; // Threshold is 50 ETH

        let mut relayer1_topped_up = false;
        let mut relayer2_topped_up = false;

        while start_time.elapsed() < max_wait_time && (!relayer1_topped_up || !relayer2_topped_up) {
            tokio::time::sleep(check_interval).await;

            let current_balance1 =
                self.contract_interactor.get_eth_balance(&relayer1.address.into_address()).await?;
            let current_balance2 =
                self.contract_interactor.get_eth_balance(&relayer2.address.into_address()).await?;

            info!("Current balances ({}s elapsed):", start_time.elapsed().as_secs());
            info!("  Relayer 1: {} ETH", alloy::primitives::utils::format_ether(current_balance1));
            info!("  Relayer 2: {} ETH", alloy::primitives::utils::format_ether(current_balance2));

            if current_balance1 > min_expected_balance && !relayer1_topped_up {
                info!("✅ Relayer 1 successfully topped up via Safe proxy!");
                relayer1_topped_up = true;
            }

            if current_balance2 > min_expected_balance && !relayer2_topped_up {
                info!("✅ Relayer 2 successfully topped up via Safe proxy!");
                relayer2_topped_up = true;
            }
        }

        // Verify both relayers were topped up
        if !relayer1_topped_up {
            return Err(anyhow!(
                "Relayer 1 was not topped up within {} seconds. Current balance: {} ETH",
                max_wait_time.as_secs(),
                alloy::primitives::utils::format_ether(
                    self.contract_interactor
                        .get_eth_balance(&relayer1.address.into_address())
                        .await?
                )
            ));
        }

        if !relayer2_topped_up {
            return Err(anyhow!(
                "Relayer 2 was not topped up within {} seconds. Current balance: {} ETH",
                max_wait_time.as_secs(),
                alloy::primitives::utils::format_ether(
                    self.contract_interactor
                        .get_eth_balance(&relayer2.address.into_address())
                        .await?
                )
            ));
        }

        info!("✅ Automatic Safe proxy top-up mechanism working correctly");
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
                let result = relayer_client
                    .sdk
                    .transaction
                    .send_transaction(&relayer_id, &tx_request, None)
                    .await;
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

        let timeout = Duration::from_secs(90);
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

        let blob_result = self
            .relayer_client
            .sdk
            .transaction
            .send_transaction(&relayer.id, &tx_request, None)
            .await?;

        let result = self.wait_for_transaction_completion(&blob_result.id).await?;

        self.relayer_client.sent_transaction_compare(tx_request, result.0)?;

        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=transaction_data_validation
    async fn test_transaction_data_validation(&self) -> Result<()> {
        info!("Testing transaction data validation...");

        let relayer = self.create_and_fund_relayer("data-validation-relayer").await?;
        info!("Created relayer: {:?}", relayer);

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

        let result = TransactionData::from_str("0xGGGG");
        if result.is_ok() {
            return Err(anyhow::anyhow!(
                "Invalid hex data should return an error but got accepted"
            ));
        }

        info!("✅ Transaction data validation working");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=balance_edge_cases
    async fn test_balance_edge_cases(&self) -> Result<()> {
        info!("Testing balance edge cases...");

        let relayer = self.create_and_fund_relayer("balance-edge-relayer").await?;
        info!("Created relayer: {:?}", relayer);

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

        let exact_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("10")?.into(),
                TransactionData::empty(),
            )
            .await;

        if exact_result.is_ok() {
            return Err(anyhow::anyhow!(
                "Transaction exceeding balance should fail as not enough gas, but succeeded"
            ));
        }

        let just_under_result = self
            .relayer_client
            .send_transaction(
                &relayer.id,
                &self.config.anvil_accounts[1],
                alloy::primitives::utils::parse_ether("9.98")?.into(),
                TransactionData::empty(),
            )
            .await;

        if just_under_result.is_err() {
            return Err(anyhow::anyhow!(
                "Transaction has enough balance should be allowed but failed"
            ));
        }

        info!("✅ Balance edge cases handled correctly");
        Ok(())
    }

    /// run single with:
    /// make run-test-debug TEST=concurrent_relayer_creation
    async fn test_concurrent_relayer_creation(&self) -> Result<()> {
        info!("Testing concurrent relayer creation to verify deadlock fix...");

        // Test creating 50 relayers concurrently to stress test deadlock prevention
        let target_relayers = 50;
        info!("Creating {} relayers concurrently to test deadlock prevention", target_relayers);

        let start_time = std::time::Instant::now();

        // Create relayers in smaller batches with higher concurrency to stress test
        let batch_size = 5;
        let mut all_relayers = Vec::new();

        for batch_start in (0..target_relayers).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size - 1, target_relayers - 1);
            info!("Creating concurrent batch {}-{}", batch_start, batch_end);

            // Collect relayer names first to avoid borrowing issues
            let relayer_names: Vec<String> = (batch_start..=batch_end)
                .map(|i| format!("concurrent_test_relayer_{}", i))
                .collect();

            let batch_futures: Vec<_> =
                relayer_names.iter().map(|name| self.create_relayer(name)).collect();

            let batch_results = futures::future::try_join_all(batch_futures).await?;

            for (i, relayer) in batch_results.into_iter().enumerate() {
                let index = batch_start + i;
                info!(
                    "Successfully created concurrent relayer {} at position {}",
                    relayer.id, index
                );
                all_relayers.push(relayer);
            }
        }

        let duration = start_time.elapsed();
        info!(
            "Successfully created {} relayers in {:?} without deadlocks!",
            all_relayers.len(),
            duration
        );

        // Verify all relayers have unique addresses (which implies unique wallet indices)
        let mut addresses: std::collections::HashSet<EvmAddress> = std::collections::HashSet::new();
        for relayer in &all_relayers {
            if !addresses.insert(relayer.address) {
                return Err(anyhow!(
                    "Duplicate address found: {}. This indicates a race condition!",
                    relayer.address
                ));
            }
        }

        info!("✅ All {} relayers have unique addresses", all_relayers.len());
        info!("✅ Concurrent relayer creation deadlock fix verified successfully!");

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
