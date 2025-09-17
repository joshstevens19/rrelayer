use alloy::sol_types::SolCall;
use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::TransactionReceipt,
    signers::local::PrivateKeySigner,
    sol,
    transports::http::{Client, Http},
};
use anyhow::{Context, Result};
use rand;
use rrelayer_core::common_types::EvmAddress;
use std::str::FromStr;
use tracing::info;

// Define a simple test contract using the sol! macro
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

pub struct ContractInteractor {
    provider: RootProvider<Http<Client>>,
    contract_address: Option<EvmAddress>,
}

impl ContractInteractor {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().context("Invalid RPC URL")?);

        Ok(Self { provider, contract_address: None })
    }

    /// Deploy the test contract using Alloy's sol! macro
    pub async fn deploy_test_contract(&mut self, deployer_private_key: &str) -> Result<Address> {
        // Create signer with the provided private key
        let signer: PrivateKeySigner =
            deployer_private_key.parse().context("Invalid private key")?;
        let wallet = EthereumWallet::from(signer);

        // Create provider with wallet for deployment
        let provider_url_str = self.provider.client().transport().url();
        let deploy_provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(provider_url_str.parse().context("Failed to parse provider URL")?);

        info!("Deploying test contract...");

        // Start continuous mining in background to allow deployment to complete
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

        // Deploy the contract using Alloy's generated deploy method
        let contract = TestContract::deploy(&deploy_provider)
            .await
            .context("Failed to deploy test contract")?;

        let contract_address = *contract.address();
        self.contract_address = Some(contract_address.into());
        info!("Test contract deployed to: {:?}", contract_address);

        // Set a random initial value to test the contract works
        let random_value = rand::random::<u32>() % 1000;
        info!("Setting initial random value: {}", random_value);

        // Create contract instance at the deployed address
        let deployed_contract = TestContract::new(contract_address, &deploy_provider);
        let set_value_call = deployed_contract.setNumber(U256::from(random_value));
        let pending_tx = set_value_call.send().await.context("Failed to set initial value")?;

        let tx_hash = *pending_tx.tx_hash();
        info!("Initial setValue transaction sent: {:?}", tx_hash);

        // Stop mining task
        let _ = tx.send(()).await;
        mining_task.abort();

        info!("✅ Test contract deployed to: {:?}", contract_address);

        Ok(contract_address)
    }

    /// Generate calldata for a simple function call (e.g., setNumber)
    pub fn encode_simple_call(&self, value: u32) -> Result<String> {
        let call = TestContract::setNumberCall { newNumber: U256::from(value) };
        let encoded = call.abi_encode();
        Ok(format!("0x{}", alloy::hex::encode(encoded)))
    }

    /// Generate calldata for a function that always reverts
    pub fn encode_always_revert(&self) -> Result<String> {
        // Function selector for a revert function
        Ok("0xabcd1234".to_string())
    }

    /// Generate calldata for a gas-intensive operation  
    pub fn encode_gas_intensive_operation(&self, iterations: u32) -> Result<String> {
        // Function selector + iterations parameter
        Ok(format!("0xef123456{:064x}", iterations))
    }

    /// Get the contract address
    pub fn contract_address(&self) -> Option<EvmAddress> {
        self.contract_address
    }

    /// Wait for a transaction to be mined and get its receipt
    pub async fn wait_for_transaction(&self, tx_hash: &str) -> Result<Option<TransactionReceipt>> {
        let hash: FixedBytes<32> = tx_hash.parse().context("Invalid transaction hash")?;

        // Poll for transaction receipt
        for _ in 0..30 {
            // Wait up to 30 seconds
            if let Some(receipt) = self.provider.get_transaction_receipt(hash).await? {
                info!("Transaction {} mined in block {:?}", tx_hash, receipt.block_number);
                return Ok(Some(receipt));
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Ok(None)
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    pub async fn get_eth_balance(&self, address: &EvmAddress) -> Result<U256> {
        let balance = self.provider.get_balance(address.into_address()).await?;
        Ok(balance)
    }

    pub async fn verify_contract_deployed(&self) -> Result<bool> {
        if let Some(address) = self.contract_address {
            let code = self.provider.get_code_at(address.into_address()).await?;
            Ok(!code.is_empty())
        } else {
            Ok(false)
        }
    }

    /// Mine a single block to confirm pending transactions
    pub async fn mine_block(&self) -> Result<()> {
        use reqwest::Client;

        let client = Client::new();
        let provider_url = self.provider.client().transport().url();

        let mine_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "anvil_mine",
            "params": [1],
            "id": 9999
        });

        let response = client
            .post(provider_url)
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
}
