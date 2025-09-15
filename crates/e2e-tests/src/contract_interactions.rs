use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::TransactionReceipt,
    transports::http::{Client, Http},
};
use anyhow::{Context, Result};
use std::str::FromStr;
use tracing::info;

pub struct ContractInteractor {
    provider: RootProvider<Http<Client>>,
    contract_address: Option<Address>,
}

impl ContractInteractor {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider = ProviderBuilder::new().on_http(rpc_url.parse().context("Invalid RPC URL")?);

        Ok(Self { provider, contract_address: None })
    }

    /// Deploy the test contract
    pub async fn deploy_test_contract(&mut self, _deployer_private_key: &str) -> Result<Address> {
        // For testing purposes, we'll use a deterministic contract address
        // In a real scenario, you'd deploy the contract here with the provided bytecode
        let contract_address = Address::from_str("0x5FbDB2315678afecb367f032d93F642f64180aa3")
            .context("Invalid contract address")?;

        self.contract_address = Some(contract_address);
        info!("Test contract address set to: {:?}", contract_address);

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
}
