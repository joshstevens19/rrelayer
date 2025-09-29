use alloy::sol_types::SolCall;
use alloy::{
    network::{AnyNetwork, EthereumWallet},
    primitives::{Address, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
};
use anyhow::{Context, Result};
use rand;
use rrelayer_core::common_types::EvmAddress;
use tracing::info;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode="6080806040523460135760df908160198239f35b600080fdfe6080806040526004361015601257600080fd5b60003560e01c9081633fb5c1cb1460925781638381f58a146079575063d09de08a14603c57600080fd5b3460745760003660031901126074576000546000198114605e57600101600055005b634e487b7160e01b600052601160045260246000fd5b600080fd5b3460745760003660031901126074576020906000548152f35b34607457602036600319011260745760043560005500fea2646970667358221220e978270883b7baed10810c4079c941512e93a7ba1cd1108c781d4bc738d9090564736f6c634300081a0033")]
    contract TestContract {
        uint256 public number;

        function setNumber(uint256 newNumber) public {
            number = newNumber;
        }

        function increment() public {
            number++;
        }
    }
}

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract TestToken {
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

pub struct ContractInteractor {
    provider: DynProvider<AnyNetwork>,
    contract_address: Option<EvmAddress>,
    token_address: Option<EvmAddress>,
}

impl ContractInteractor {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider = ProviderBuilder::new()
            .network::<AnyNetwork>()
            .connect(rpc_url)
            .await
            .context("Failed to connect to RPC")?;

        Ok(Self {
            provider: DynProvider::new(provider),
            contract_address: None,
            token_address: None,
        })
    }

    pub async fn deploy_test_contract(&mut self, deployer_private_key: &str) -> Result<Address> {
        let signer: PrivateKeySigner =
            deployer_private_key.parse().context("Invalid private key")?;
        let wallet = EthereumWallet::from(signer);

        let provider_url_str = "http://localhost:8545";
        let deploy_provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect(provider_url_str)
            .await
            .context("Failed to connect to provider")?;

        info!("Deploying test contract...");

        let mining_url = provider_url_str.to_string();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        let mining_task = tokio::spawn(async move {
            let client = reqwest::Client::new();
            loop {
                tokio::select! {
                    _ = rx.recv() => {
                        info!("Stopping mining task");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                        let mine_request = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "anvil_mine",
                            "params": [1],
                            "id": 9999
                        });

                        let _ = client
                            .post(&mining_url)
                            .header("Content-Type", "application/json")
                            .json(&mine_request)
                            .send()
                            .await;
                    }
                }
            }
        });

        let contract = TestContract::deploy(&deploy_provider)
            .await
            .context("Failed to deploy test contract")?;

        let contract_address = *contract.address();
        self.contract_address = Some(contract_address.into());
        info!("Test contract deployed to: {:?}", contract_address);

        let random_value = rand::random::<u32>() % 1000;
        info!("Setting initial random value: {}", random_value);

        let deployed_contract = TestContract::new(contract_address, &deploy_provider);
        let set_value_call = deployed_contract.setNumber(U256::from(random_value));
        let pending_tx = set_value_call.send().await.context("Failed to set initial value")?;

        let tx_hash = *pending_tx.tx_hash();
        info!("Initial setValue transaction sent: {:?}", tx_hash);

        let _ = tx.send(()).await;
        mining_task.abort();

        info!("[SUCCESS] Test contract deployed to: {:?}", contract_address);

        // Wait longer to ensure deployment is fully settled and nonce is incremented
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        Ok(contract_address)
    }

    pub async fn deploy_test_token(&mut self, deployer_private_key: &str) -> Result<Address> {
        info!("Deploying ERC-20 test token using Forge...");

        let rpc_url = "http://localhost:8545".to_string(); // Use default URL

        let contracts_dir = std::path::Path::new("contracts");
        if !contracts_dir.exists() {
            return Err(anyhow::anyhow!(
                "Contracts directory not found at {:?}",
                contracts_dir.canonicalize()
            ));
        }

        info!("Using RPC URL: {} and contracts dir: {:?}", rpc_url, contracts_dir.canonicalize());

        // Wait longer to ensure any previous transactions are settled and nonces are updated
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        let client = reqwest::Client::new();
        let auto_mine_enable = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_setAutomine",
            "params": [true],
            "id": 1
        });

        let _ = client
            .post(&rpc_url)
            .header("Content-Type", "application/json")
            .json(&auto_mine_enable)
            .send()
            .await;

        info!("Enabled auto-mining for forge deployment");

        let deployer_key = deployer_private_key.to_string();
        let rpc_url_clone = rpc_url.clone();
        let forge_task = tokio::task::spawn_blocking(move || {
            std::process::Command::new("forge")
                .arg("script")
                .arg("script/DeployMyCustomToken.s.sol:DeployMyCustomToken")
                .arg("--rpc-url")
                .arg(&rpc_url_clone)
                .arg("--private-key")
                .arg(&deployer_key)
                .arg("--broadcast")
                .current_dir("contracts")
                .output()
        });

        let output = tokio::time::timeout(tokio::time::Duration::from_secs(30), forge_task)
            .await
            .context("Forge deployment timed out")?
            .context("Failed to execute forge task")?
            .context("Failed to run forge script")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Forge deployment failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let token_address = self
            .extract_deployed_address(&stdout)
            .context("Failed to extract deployed contract address from forge output")?;

        self.token_address = Some(token_address.into());
        info!("ERC-20 test token deployed to: {:?}", token_address);

        let token_contract = TestToken::new(token_address, &self.provider);
        let total_supply =
            token_contract.totalSupply().call().await.context("Failed to verify token contract")?;

        info!("[SUCCESS] Test ERC-20 token verified - Total supply: {}", total_supply);

        let fund_addresses = vec![
            // raw, aws secret manager and gcp secret manager
            "0x655B2B8861D7E911D283A05A5CAD042C157106DA",
            // privy
            "0xa93e13Db16BF70b3D6B828bC0185A9F3AdD44BA9",
            // KMS
            "0x33993A4F4AA617DA4558A0CFD0C39A7989B67720",
        ];

        for item in fund_addresses {
            // Transfer tokens from deployer to the automatic top-up funding address
            // The deployer (anvil_accounts[0]) has all the tokens, but the funding address in YAML is different
            // Use the known funding address from the config
            let funding_address: Address =
                item.parse().context("Failed to parse funding address")?;

            let transfer_amount = U256::from(100_000u64) * U256::from(10u64).pow(U256::from(18u64));

            info!(
                "Transferring {} tokens from deployer to funding address {:?}",
                alloy::primitives::utils::format_units(transfer_amount, 18)
                    .unwrap_or("N/A".to_string()),
                funding_address
            );

            self.transfer_tokens(&funding_address, transfer_amount, deployer_private_key)
                .await
                .context("Failed to transfer tokens to funding address")?;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let auto_mine_disable = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_setAutomine",
            "params": [false],
            "id": 2
        });

        let _ = client
            .post(&rpc_url)
            .header("Content-Type", "application/json")
            .json(&auto_mine_disable)
            .send()
            .await;

        info!("Disabled auto-mining after forge deployment");

        Ok(token_address)
    }

    pub async fn deploy_safe_contracts(&mut self, deployer_private_key: &str) -> Result<Address> {
        info!("Deploying Safe contracts using Forge...");

        let rpc_url = "http://localhost:8545".to_string(); // Use default URL

        let contracts_dir = std::path::Path::new("contracts");
        if !contracts_dir.exists() {
            return Err(anyhow::anyhow!(
                "Contracts directory not found at {:?}",
                contracts_dir.canonicalize()
            ));
        }

        info!("Using RPC URL: {} and contracts dir: {:?}", rpc_url, contracts_dir.canonicalize());

        // Wait to ensure any previous transactions are settled
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        let client = reqwest::Client::new();
        let auto_mine_enable = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_setAutomine",
            "params": [true],
            "id": 1
        });

        let _ = client
            .post(&rpc_url)
            .header("Content-Type", "application/json")
            .json(&auto_mine_enable)
            .send()
            .await;

        info!("Enabled auto-mining for Safe deployment");

        let deployer_key = deployer_private_key.to_string();
        let rpc_url_clone = rpc_url.clone();
        let forge_task = tokio::task::spawn_blocking(move || {
            std::process::Command::new("forge")
                .arg("script")
                .arg("script/DeploySafe.s.sol:DeploySafe")
                .arg("--rpc-url")
                .arg(&rpc_url_clone)
                .arg("--private-key")
                .arg(&deployer_key)
                .arg("--broadcast")
                .current_dir("contracts")
                .output()
        });

        let output = tokio::time::timeout(tokio::time::Duration::from_secs(30), forge_task)
            .await
            .context("Safe deployment timed out")?
            .context("Failed to execute Safe forge task")?
            .context("Failed to run Safe forge script")?;

        let auto_mine_disable = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_setAutomine",
            "params": [false],
            "id": 2
        });

        let _ = client
            .post(&rpc_url)
            .header("Content-Type", "application/json")
            .json(&auto_mine_disable)
            .send()
            .await;

        info!("Disabled auto-mining after Safe deployment");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Safe deployment failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let safe_address = self
            .extract_safe_address(&stdout)
            .context("Failed to extract deployed Safe address from forge output")?;

        info!("Safe contracts deployed - Safe proxy address: {:?}", safe_address);

        // Validate the Safe deployed to the expected address based on the provider
        let expected_address = self.get_expected_safe_address_for_provider()?;

        if safe_address != expected_address {
            return Err(anyhow::anyhow!(
                "Safe deployment address mismatch! Expected: {:?}, Got: {:?}",
                expected_address,
                safe_address
            ));
        }

        info!(
            "[SUCCESS] Safe proxy deployed to expected deterministic address: {:?}",
            safe_address
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(safe_address)
    }

    fn extract_safe_address(&self, forge_output: &str) -> Result<Address> {
        for line in forge_output.lines() {
            if line.contains("Safe Proxy deployed to:") {
                if let Some(addr_str) = line.split("Safe Proxy deployed to: ").nth(1) {
                    if let Some(addr) = addr_str.split_whitespace().next() {
                        return addr.parse().context("Failed to parse Safe address");
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Could not find deployed Safe address in forge output"))
    }

    pub fn get_expected_safe_address_for_provider(&self) -> Result<Address> {
        // Check the SAFE_OWNER_ADDRESS environment variable to determine which provider is being used
        let safe_owner_address = std::env::var("SAFE_OWNER_ADDRESS")
            .unwrap_or_else(|_| "0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf".to_string());

        // Map owner addresses to their expected Safe deployment addresses
        let expected_address = match safe_owner_address.to_lowercase().as_str() {
            "0x1c073e63f70701bc545019d3c4f2a25a69eca8cf" => {
                // Raw provider
                "0xcfe267de230a234c5937f18f239617b7038ec271"
            }
            "0xde3d9699427d15d0a1419736141997e352f10f61" => {
                // Privy provider
                "0xd9fa512bc7ec216f0c01f4de4232629f3ec3bac7"
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown Safe owner address: {}. Cannot determine expected Safe address.",
                    safe_owner_address
                ));
            }
        };

        expected_address.parse().context("Failed to parse expected Safe address")
    }

    fn extract_deployed_address(&self, forge_output: &str) -> Result<Address> {
        for line in forge_output.lines() {
            if line.contains("TestToken deployed to:") {
                if let Some(addr_str) = line.split("TestToken deployed to: ").nth(1) {
                    if let Some(addr) = addr_str.split_whitespace().next() {
                        return addr.parse().context("Failed to parse contract address");
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Could not find deployed contract address in forge output"))
    }

    pub async fn transfer_tokens(
        &self,
        to_address: &Address,
        amount: U256,
        from_private_key: &str,
    ) -> Result<()> {
        let token_address = self.token_address.context("Token not deployed yet")?;

        let signer: PrivateKeySigner = from_private_key.parse().context("Invalid private key")?;
        let wallet = EthereumWallet::from(signer);

        let provider_url_str = "http://localhost:8545"; // Use default URL
        let transfer_provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect(provider_url_str)
            .await
            .context("Failed to connect to provider")?;

        let token_contract = TestToken::new(token_address.into(), &transfer_provider);
        let transfer_call = token_contract.transfer(*to_address, amount);
        let pending_tx = transfer_call.send().await.context("Failed to transfer tokens")?;

        info!("Transferred {} tokens to {:?}, tx: {:?}", amount, to_address, pending_tx.tx_hash());

        Ok(())
    }

    pub async fn get_token_balance(&self, address: &Address) -> Result<U256> {
        let token_address = self.token_address.context("Token not deployed yet")?;

        let token_contract = TestToken::new(token_address.into(), &self.provider);
        let balance = token_contract
            .balanceOf(*address)
            .call()
            .await
            .context("Failed to get token balance")?;

        Ok(balance)
    }

    pub fn encode_simple_call(&self, value: u32) -> Result<String> {
        let call = TestContract::setNumberCall { newNumber: U256::from(value) };
        let encoded = call.abi_encode();
        Ok(format!("0x{}", alloy::hex::encode(encoded)))
    }

    pub fn contract_address(&self) -> Option<EvmAddress> {
        self.contract_address
    }

    pub fn token_address(&self) -> Option<EvmAddress> {
        self.token_address
    }

    pub async fn verify_contract_deployed(&self) -> Result<bool> {
        if let Some(address) = self.contract_address {
            let code = self.provider.get_code_at(address.into()).await?;
            Ok(!code.is_empty())
        } else {
            Ok(false)
        }
    }

    pub async fn get_eth_balance(&self, address: &Address) -> Result<U256> {
        self.provider.get_balance(*address).await.context("Failed to get ETH balance")
    }
}
