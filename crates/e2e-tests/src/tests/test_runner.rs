use crate::client::{E2ETestConfig, RelayerClient};
use crate::infrastructure::{AnvilManager, ContractInteractor, WebhookTestServer};
use crate::tests::registry;
use crate::tests::registry::TestRegistry;
use crate::tests::test_suite::{TestInfo, TestResult, TestSuite};
use alloy::network::{AnyTransactionReceipt, EthereumWallet};
use alloy::primitives::U256;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::{anyhow, Context};
use rrelayer_core::common_types::EvmAddress;
use rrelayer_core::relayer::CreateRelayerResult;
use rrelayer_core::transaction::types::{Transaction, TransactionId, TransactionStatus};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{error, info};

pub struct TestRunner {
    pub config: E2ETestConfig,
    pub relayer_client: RelayerClient,
    pub contract_interactor: ContractInteractor,
    pub anvil_manager: AnvilManager,
    pub webhook_server: Option<WebhookTestServer>,
    relayer_counter: std::sync::atomic::AtomicUsize,
    created_relayers: std::sync::Mutex<Vec<CreateRelayerResult>>,
}

impl TestRunner {
    pub async fn new(config: E2ETestConfig, anvil_manager: AnvilManager) -> anyhow::Result<Self> {
        let relayer_client = RelayerClient::new(&config);

        let anvil_url = format!("http://127.0.0.1:{}", config.anvil_port);
        let mut contract_interactor = ContractInteractor::new(&anvil_url).await?;

        let deployer_private_key = &config.anvil_private_keys[0];
        let contract_address =
            contract_interactor.deploy_test_contract(deployer_private_key).await?;

        info!("Test contract deployed at: {:?}", contract_address);

        let token_address = contract_interactor.deploy_test_token(deployer_private_key).await?;

        info!("[SUCCESS] Test ERC-20 token deployed at: {:?}", token_address);

        let safe_address = contract_interactor.deploy_safe_contracts(deployer_private_key).await?;

        info!("[SUCCESS] Safe contracts deployed - Safe proxy at: {:?}", safe_address);

        let webhook_server = Some(WebhookTestServer::new("test-webhook-secret-123".to_string()));

        Ok(Self {
            config,
            relayer_client,
            contract_interactor,
            anvil_manager,
            webhook_server,
            relayer_counter: std::sync::atomic::AtomicUsize::new(0),
            created_relayers: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub fn into_anvil_manager(self) -> AnvilManager {
        self.anvil_manager
    }

    pub async fn start_webhook_server(&self) -> anyhow::Result<()> {
        if let Some(webhook_server) = &self.webhook_server {
            let server = webhook_server.clone();
            tokio::spawn(async move {
                if let Err(e) = server.start(8546).await {
                    error!("Webhook server failed to start: {}", e);
                }
            });

            tokio::time::sleep(Duration::from_millis(500)).await;
            info!("[SUCCESS] Webhook test server started on port 8546");
        }
        Ok(())
    }

    pub fn stop_webhook_server(&self) {
        if let Some(webhook_server) = &self.webhook_server {
            webhook_server.stop();
            info!("[STOP] Webhook test server stopped");
        }
    }

    pub fn webhook_server(&self) -> Option<&WebhookTestServer> {
        self.webhook_server.as_ref()
    }

    pub async fn mine_blocks(&self, num_blocks: u64) -> anyhow::Result<()> {
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

        info!("[MINING] Mined {} blocks", num_blocks);
        Ok(())
    }

    pub async fn mine_and_wait(&self) -> anyhow::Result<()> {
        self.mine_blocks(1).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    pub async fn fund_relayer(
        &self,
        relayer_address: &EvmAddress,
        funding_amount: U256,
    ) -> anyhow::Result<()> {
        let anvil_url = format!("http://127.0.0.1:{}", self.config.anvil_port);

        let private_key = self.config.anvil_private_keys[0].clone();
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet = EthereumWallet::from(signer);

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

        self.mine_and_wait().await?;

        let receipt =
            pending_tx.get_receipt().await.context("Failed to get funding transaction receipt")?;

        info!("Funding transaction mined in block: {:?}", receipt.block_number);
        info!("Successfully funded relayer {} with 10 ETH", relayer_address);

        Ok(())
    }

    pub async fn create_relayer(&self, name: &str) -> anyhow::Result<CreateRelayerResult> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        {
            let mut relayers = self.created_relayers.lock().unwrap();
            relayers.push(relayer.clone());
        }

        self.relayer_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(relayer)
    }

    pub async fn create_by_index_and_fund_relayer(
        &self,
        target_index: usize,
    ) -> anyhow::Result<CreateRelayerResult> {
        {
            let relayers = self.created_relayers.lock().unwrap();
            if target_index < relayers.len() {
                info!("Returning existing relayer at index {}", target_index);
                let relayer = relayers[target_index].clone();
                self.fund_relayer(&relayer.address, alloy::primitives::utils::parse_ether("10")?)
                    .await?;
                return Ok(relayers[target_index].clone());
            }
        }

        let current_count = self.relayer_counter.load(std::sync::atomic::Ordering::SeqCst);
        let batch_size = 10;
        let total_to_create = target_index - current_count + 1;

        info!(
            "Creating {} relayers from index {} to {} in batches of {}",
            total_to_create, current_count, target_index, batch_size
        );

        for batch_start in (current_count..=target_index).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size - 1, target_index);
            info!("Creating relayers batch {}-{}", batch_start, batch_end);

            if batch_start <= target_index && target_index <= batch_end {
                for i in batch_start..=batch_end {
                    let relayer_name = format!("relayer_{}", i);
                    let relayer = if i == target_index {
                        self.create_and_fund_relayer(&relayer_name).await?
                    } else {
                        self.create_relayer(&relayer_name).await?
                    };

                    info!("Created relayer {} at index {}", relayer.id, i);
                }
            } else {
                let relayer_names: Vec<String> =
                    (batch_start..=batch_end).map(|i| format!("relayer_{}", i)).collect();

                let batch_futures: Vec<_> =
                    relayer_names.iter().map(|name| self.create_relayer(name)).collect();

                let _batch_results = futures::future::try_join_all(batch_futures).await?;

                for i in batch_start..=batch_end {
                    info!("Created relayer at index {}", i);
                }
            }
        }

        let relayers = self.created_relayers.lock().unwrap();
        if target_index < relayers.len() {
            Ok(relayers[target_index].clone())
        } else {
            Err(anyhow!("Failed to create relayer at target index {}", target_index))
        }
    }

    pub async fn create_and_fund_relayer(&self, name: &str) -> anyhow::Result<CreateRelayerResult> {
        let relayer = self
            .relayer_client
            .create_relayer(name, self.config.chain_id)
            .await
            .context("Failed to create relayer")?;

        self.fund_relayer(&relayer.address, alloy::primitives::utils::parse_ether("10")?)
            .await
            .context("Failed to fund relayer")?;

        // Store the created relayer
        {
            let mut relayers = self.created_relayers.lock().unwrap();
            relayers.push(relayer.clone());
        }

        self.relayer_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(relayer)
    }

    pub async fn wait_for_transaction_completion(
        &self,
        transaction_id: &TransactionId,
    ) -> anyhow::Result<(Transaction, AnyTransactionReceipt)> {
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
                TransactionStatus::CONFIRMED | TransactionStatus::MINED => {
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
                TransactionStatus::FAILED => {
                    anyhow::bail!("Transaction {} failed: {:?}", transaction_id, result);
                }
                TransactionStatus::PENDING | TransactionStatus::INMEMPOOL => {
                    info!(
                        "Transaction {} still pending, mining a block and waiting...",
                        transaction_id
                    );
                    self.mine_and_wait().await?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                TransactionStatus::EXPIRED => {
                    anyhow::bail!("Transaction {} expired: {:?}", transaction_id, result);
                }
            }
        }
    }

    pub async fn run_all_tests(&mut self) -> TestSuite {
        println!("[START] RRelayer E2E Test Suite");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.start_webhook_server().await.unwrap();
        let relayer = self.create_and_fund_relayer("automatic_top_up").await.unwrap();
        self.fund_relayer(
            &relayer.address,
            alloy::primitives::utils::parse_ether("100000000").unwrap(),
        )
        .await
        .unwrap();

        info!("[CHECK] Testing webhook server accessibility...");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();

        match client.get("http://localhost:8546/health").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("[SUCCESS] Webhook server is accessible at http://localhost:8546");
                } else {
                    info!("[WARNING] Webhook server responded with status: {}", response.status());
                }
            }
            Err(e) => {
                info!("[ERROR] Webhook server not accessible: {}", e);
                info!("Continuing test without accessibility verification...");
            }
        }

        let mut suite = TestSuite::new("RRelayer E2E Tests".to_string());
        let overall_start = Instant::now();

        let registry_tests = TestRegistry::get_all_tests();

        for test_def in registry_tests {
            let test_result = self.run_registry_test(&test_def).await;
            suite.add_test(test_result);
            self.mine_and_wait().await.expect("Could not mine block");
        }

        self.stop_webhook_server();

        suite.duration = overall_start.elapsed();
        self.print_final_report(&suite);
        suite
    }

    pub async fn run_filtered_test(&mut self, test_name: &str) -> TestSuite {
        println!("[START] RRelayer E2E Test Suite - Single Test: {}", test_name);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.start_webhook_server().await.unwrap();
        let relayer = self.create_and_fund_relayer("startup").await.unwrap();
        self.fund_relayer(
            &relayer.address,
            alloy::primitives::utils::parse_ether("100000000").unwrap(),
        )
        .await
        .unwrap();

        info!("[CHECK] Testing webhook server accessibility...");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap();

        match client.get("http://localhost:8546/health").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("[SUCCESS] Webhook server is accessible at http://localhost:8546");
                } else {
                    info!("[WARNING] Webhook server responded with status: {}", response.status());
                }
            }
            Err(e) => {
                info!("[ERROR] Webhook server not accessible: {}", e);
                info!("Continuing test without accessibility verification...");
            }
        }

        let mut suite = TestSuite::new(format!("Single Test: {}", test_name));
        let overall_start = Instant::now();

        let registry_tests = TestRegistry::get_all_tests();

        if let Some(test_def) = registry_tests.iter().find(|t| t.name == test_name) {
            let test_result = self.run_registry_test(test_def).await;
            suite.add_test(test_result);
        }

        self.stop_webhook_server();

        suite.duration = overall_start.elapsed();
        self.print_final_report(&suite);
        suite
    }

    async fn run_registry_test(&mut self, test_def: &registry::TestDefinition) -> TestInfo {
        print!("[TEST] {} ... ", test_def.description);
        let start = Instant::now();

        let webhook_server =
            self.webhook_server().ok_or_else(|| anyhow!("Webhook server not initialized")).unwrap();

        webhook_server.clear_webhooks();

        let result = timeout(Duration::from_secs(180), (test_def.function)(self)).await;

        let test_result = match result {
            Ok(Ok(())) => {
                println!("[SUCCESS] PASS");
                TestResult::Passed
            }
            Ok(Err(e)) => {
                println!("[ERROR] FAIL");
                TestResult::Failed(e.to_string())
            }
            Err(_) => {
                println!("[TIMEOUT] TIMEOUT");
                TestResult::Timeout
            }
        };

        let duration = start.elapsed();
        TestInfo::new(test_def.name.to_string(), test_result, duration)
    }

    fn print_final_report(&self, suite: &TestSuite) {
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let passed = suite.passed_count();
        let failed = suite.failed_count();
        let timeout = suite.timeout_count();
        let skipped = suite.skipped_count();
        let total = suite.total_count();

        if failed == 0 && timeout == 0 {
            println!("[SUCCESS] Test Suites: 1 passed, 1 total");
            println!("[SUCCESS] Tests:       {} passed, {} total", passed, total);
        } else {
            println!(
                "[ERROR] Test Suites: {} failed, 1 total",
                if failed > 0 || timeout > 0 { 1 } else { 0 }
            );
            println!(
                "[ERROR] Tests:       {} failed, {} passed, {} total",
                failed + timeout,
                passed,
                total
            );
        }

        if skipped > 0 {
            println!("[SKIP] Skipped:     {}", skipped);
        }

        println!("[TIME] Time:        {:.2}s", suite.duration.as_secs_f64());

        if failed > 0 || timeout > 0 {
            println!();
            println!("Failed Tests:");
            for test in &suite.tests {
                if let TestResult::Failed(msg) = &test.result {
                    println!("  [ERROR] {} - {}", test.name, msg);
                } else if let TestResult::Timeout = &test.result {
                    println!("  [TIMEOUT] {} - Test timed out after 180 seconds", test.name);
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
}
