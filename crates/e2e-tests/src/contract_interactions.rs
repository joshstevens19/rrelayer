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
use std::str::FromStr;
use tracing::info;

// Define a simple test contract using the sol! macro
sol! {
    #[allow(missing_docs)]
    #[sol(rpc, bytecode="608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c8063209652551461003b5780635524107714610059575b600080fd5b610043610071565b60405161005091906100d1565b60405180910390f35b61006f600480360381019061006a91906100fd565b610077565b005b60005481565b8060008190555050565b6000819050919050565b61009481610081565b82525050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b60006100c58261009a565b9050919050565b6100d5816100ba565b82525050565b60006020820190506100f0600083018461008b565b92915050565b60006020828403121561010c5761010b61012a565b5b600061011a84828501610113565b91505092915050565b61012c81610081565b811461013757600080fd5b50565b60008135905061014981610123565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806101a057607f821691505b6020821081036101b3576101b2610171565b5b5091905056fea2646970667358221220b29e4b69e5b1a9c4a9b8a9b8a9b8a9b8a9b8a9b8a9b8a9b8a9b8a9b8a9b8a9b864736f6c63430008110033")]
    contract TestContract {
        uint256 public value;

        function setValue(uint256 newValue) public {
            value = newValue;
        }

        function getValue() public view returns (uint256) {
            return value;
        }
    }
}

pub struct ContractInteractor {
    provider: RootProvider<Http<Client>>,
    contract_address: Option<Address>,
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

        // Stop mining task
        let _ = tx.send(()).await;
        mining_task.abort();

        let contract_address = *contract.address();
        self.contract_address = Some(contract_address);

        info!("✅ Test contract deployed to: {:?}", contract_address);

        Ok(contract_address)
    }

    /// Generate calldata for a simple function call (e.g., setValue)
    pub fn encode_simple_call(&self, value: u32) -> Result<String> {
        // Simple function selector + encoded value for testing
        // This is setValue(uint256) function selector + padded value
        Ok(format!("0x55241077{:064x}", value))
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
    pub fn contract_address(&self) -> Option<Address> {
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

    /// Get current block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let block_number = self.provider.get_block_number().await?;
        Ok(block_number)
    }

    /// Get ETH balance for an address
    pub async fn get_eth_balance(&self, address: &str) -> Result<U256> {
        let addr = Address::from_str(address).context("Invalid address")?;
        let balance = self.provider.get_balance(addr).await?;
        Ok(balance)
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
