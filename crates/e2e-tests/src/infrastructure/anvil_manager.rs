use anyhow::{Context, Result};
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::info;

pub struct AnvilManager {
    port: u16,
    process: Option<Child>,
}

impl AnvilManager {
    pub async fn new(port: u16) -> Result<Self> {
        Ok(Self { port, process: None })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Anvil on port {}", self.port);

        self.kill_existing_process_on_port().await?;

        let output = Command::new("anvil").arg("--version").output().await.context(
            "Failed to execute anvil. Make sure foundry is installed and anvil is in PATH",
        )?;

        if !output.status.success() {
            anyhow::bail!(
                "Anvil is not available. Please install foundry: https://book.getfoundry.sh/"
            );
        }

        let child = Command::new("anvil")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "true")
            .args([
                "--host",
                "0.0.0.0",
                "--port",
                &self.port.to_string(),
                "--accounts",
                "10",
                "--balance",
                "10000000000",
                "--mnemonic",
                "test test test test test test test test test test test junk",
                "--no-mining", // Don't auto-mine empty blocks
                "--gas-limit",
                "30000000",
                "--gas-price",
                "1000000000",
                "--base-fee",
                "1000000000",
                "--chain-id",
                "31337",
                "--hardfork",
                "cancun",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("Failed to start anvil process")?;

        self.process = Some(child);
        info!("Anvil started successfully on port {}", self.port);

        // Wait a moment for anvil to fully start
        tokio::time::sleep(Duration::from_secs(1)).await;

        self.verify_anvil_ready().await?;

        Ok(())
    }

    async fn kill_existing_process_on_port(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use tokio::process::Command;

            let output =
                Command::new("lsof").args(["-ti", &format!(":{}", self.port)]).output().await;

            if let Ok(output) = output {
                if output.status.success() {
                    let pids = String::from_utf8_lossy(&output.stdout);
                    for pid_str in pids.trim().lines() {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            info!("Killing existing process {} on port {}", pid, self.port);
                            let _ =
                                Command::new("kill").args(["-9", &pid.to_string()]).output().await;
                        }
                    }
                    // Give processes time to die
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }

        #[cfg(windows)]
        {
            // Windows equivalent using netstat and taskkill
            let output = Command::new("netstat").args(["-ano"]).output().await;

            if let Ok(output) = output {
                let netstat_output = String::from_utf8_lossy(&output.stdout);
                for line in netstat_output.lines() {
                    if line.contains(&format!(":{}", self.port)) && line.contains("LISTENING") {
                        if let Some(pid_str) = line.split_whitespace().last() {
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                info!("Killing existing process {} on port {}", pid, self.port);
                                let _ = Command::new("taskkill")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .output()
                                    .await;
                            }
                        }
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            info!("Stopping Anvil process");
            child.kill().await.context("Failed to kill anvil process")?;
            child.wait().await.context("Failed to wait for anvil process")?;
            info!("Anvil stopped successfully");
        }
        self.kill_existing_process_on_port().await
    }

    // pub async fn restart(&mut self) -> Result<()> {
    //     info!("Restarting Anvil to ensure clean state");
    //     self.stop().await.context("Failed to stop Anvil during restart")?;
    //
    //     tokio::time::sleep(Duration::from_millis(500)).await;
    //
    //     self.start().await.context("Failed to start Anvil during restart")?;
    //
    //     info!("Anvil restarted successfully with fresh state");
    //     Ok(())
    // }

    async fn verify_anvil_ready(&self) -> Result<()> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("http://127.0.0.1:{}", self.port);

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });

        for attempt in 1..=5 {
            match client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Anvil is ready and responding");
                        return Ok(());
                    }
                }
                Err(e) => {
                    info!("Attempt {}/5 failed to connect to anvil: {}", attempt, e);
                }
            }

            if attempt < 5 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        anyhow::bail!("Failed to verify anvil is ready after 5 attempts");
    }

    async fn get_transaction_count(&self, account: &str) -> Result<u64> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("http://127.0.0.1:{}", self.port);

        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [account, "latest"],
            "id": 1
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to get transaction count")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get transaction count: {}", response.status());
        }

        let json: serde_json::Value =
            response.json().await.context("Failed to parse transaction count response")?;

        let nonce_str =
            json["result"].as_str().context("Transaction count result is not a string")?;

        let nonce_str = nonce_str.strip_prefix("0x").unwrap_or(nonce_str);
        u64::from_str_radix(nonce_str, 16).context("Failed to parse transaction count as hex")
    }

    pub async fn mine_blocks_with_transactions(&self, num_blocks: u64) -> Result<()> {
        use reqwest::Client;

        info!("[MINING]  Mining {} blocks with transactions for gas estimation data", num_blocks);

        let client = Client::new();
        let url = format!("http://127.0.0.1:{}", self.port);

        let base_gas_price = 1_000_000_000_u64; // 1 gwei
        let sender_account = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"; // First Anvil account

        let mut current_nonce = self.get_transaction_count(sender_account).await?;

        for block_i in 0..num_blocks.min(20) {
            for tx_i in 0..2 {
                let gas_multiplier = 1 + ((block_i + tx_i) % 5);
                let gas_price = format!("0x{:x}", base_gas_price * gas_multiplier);

                let tx_request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_sendTransaction",
                    "params": [{
                        "from": sender_account,
                        "to": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
                        "value": "0x1000000000000000",
                        "maxFeePerGas": gas_price,
                        "maxPriorityFeePerGas": format!("0x{:x}", base_gas_price / 2),
                        "gas": "0x5208",
                        "nonce": format!("0x{:x}", current_nonce)
                    }],
                    "id": block_i * 10 + tx_i + 10
                });

                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&tx_request)
                    .send()
                    .await
                    .context("Failed to send test transaction")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    info!(
                        "Failed to send transaction {} in block {} (nonce {}): {} - {}",
                        tx_i, block_i, current_nonce, status, body
                    );
                } else {
                    current_nonce += 1;
                }

                // Small delay to ensure transaction is in mempool before mining
                tokio::time::sleep(Duration::from_millis(50)).await;

                let mine_request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "anvil_mine",
                    "params": [1],
                    "id": 1000 + block_i * 10 + tx_i
                });

                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&mine_request)
                    .send()
                    .await
                    .context("Failed to mine block")?;

                if !response.status().is_success() {
                    info!(
                        "Failed to mine block after transaction {} in block {}: {}",
                        tx_i,
                        block_i,
                        response.status()
                    );
                }

                let latest_block_request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_getBlockByNumber",
                    "params": ["latest", true],
                    "id": 2000 + block_i * 10 + tx_i
                });

                let block_response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&latest_block_request)
                    .send()
                    .await
                    .context("Failed to get latest block")?;

                if let Ok(block_data) = block_response.json::<serde_json::Value>().await {
                    let tx_count = block_data["result"]["transactions"]
                        .as_array()
                        .map(|arr| arr.len())
                        .unwrap_or(0);
                    info!(
                        "Block mined with {} transactions (block_i: {}, tx_i: {})",
                        tx_count, block_i, tx_i
                    );
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        if num_blocks > 20 {
            let remaining = num_blocks - 20;
            let mine_request = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "anvil_mine",
                "params": [remaining],
                "id": 2000
            });

            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&mine_request)
                .send()
                .await
                .context("Failed to mine remaining blocks")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                info!("Failed to mine remaining {} blocks: HTTP {} - {}", remaining, status, body);
            }
        }

        info!("[SUCCESS] Successfully mined {} blocks with gas price history (transactions in recent blocks)", num_blocks);
        Ok(())
    }

    pub async fn mine_block(&self) -> Result<()> {
        use reqwest::Client;

        let client = Client::new();
        let url = format!("http://127.0.0.1:{}", self.port);

        let mine_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_mine",
            "params": [1],
            "id": 9999
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&mine_request)
            .send()
            .await
            .context("Failed to mine block")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to mine block: {}", response.status());
        }

        Ok(())
    }

    pub async fn mine_and_wait(&self) -> Result<()> {
        self.mine_block().await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

impl Drop for AnvilManager {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
        }
    }
}
