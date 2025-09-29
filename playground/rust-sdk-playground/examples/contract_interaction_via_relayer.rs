//! Example showing how contract interactions work with RelayerProvider
//! This simulates the ERC20 transfer example but with relayer hijacking

use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    sol,
};
use eyre::Result;
use rrelayer::{
    RelayerClient, RelayerClientAuth, RelayerClientConfig, RelayerId, RelayerSigner, with_relayer,
};
use std::{str::FromStr, sync::Arc};

// Import our relayer integration
use rust_sdk_playground::alloy::{RelayerSigner, with_relayer};

// Simplified ERC20 contract interface for demo
sol! {
    #[allow(missing_docs)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🪙 ERC20 Transfer via Relayer Example");

    // 1. Setup relayer signer
    let relayer_signer = create_relayer_signer()?;
    println!("✅ Created RelayerSigner: {}", relayer_signer.address());

    // 2. Create provider and wrap with relayer hijacking
    let mock_provider = MockProvider::new();
    let hijacked_provider = with_relayer(mock_provider, relayer_signer.clone());

    println!("🔧 Provider wrapped with relayer hijacking");

    // 3. Setup contract and accounts (like original example)
    let alice = relayer_signer.address();
    let bob = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
    let token_address = Address::from_str("0xA0b86a33E6b3e96666cD9F69E1a1e7B46e5b7aEE")?; // Example USDC

    println!("\n🏦 Contract interaction setup:");
    println!("   Token: {} (USDC)", token_address);
    println!("   Alice (relayer): {}", alice);
    println!("   Bob: {}", bob);

    // 4. Create contract instance (this would work with real provider)
    println!("\n📋 Contract operations:");
    println!("   contract = IERC20::new(token_address, &hijacked_provider)");

    // 5. Read operations (these go through normal RPC)
    println!("\n🔍 Reading balances (normal RPC calls):");
    println!("   alice_balance = contract.balanceOf(alice).call()");
    println!("   bob_balance = contract.balanceOf(bob).call()");
    println!("   → These are read-only, so they use normal RPC");

    // 6. Write operation - this gets hijacked!
    let amount = U256::from(1000000); // 1 USDC (6 decimals)

    println!("\n💸 Transferring {} USDC from Alice to Bob...", amount);
    println!("   Using: contract.transfer(bob, amount).send()");

    // This would normally send to RPC, but gets hijacked by RelayerProvider!
    println!("\n🔄 [INTERCEPTED] Contract transaction being hijacked...");
    println!("   → Alloy builds: contract.transfer(bob, 1000000)");
    println!("   → RelayerProvider intercepts the .send() call");
    println!("   → Converts contract call to RelayTransactionRequest:");
    println!("     - to: {} (token contract)", token_address);
    println!("     - data: transfer(0x{}, {})", hex::encode(bob.as_slice()), amount);
    println!("   → Sends via relayer.transaction().send()");

    match simulate_contract_transaction().await {
        Ok(tx_hash) => {
            println!("✅ Contract transaction sent via relayer!");
            println!("   Transaction Hash: {}", tx_hash);
            println!("   → ERC20 transfer went through your relayer!");
        }
        Err(e) => {
            println!("❌ Failed: {} (expected without valid relayer credentials)", e);
        }
    }

    println!("\n🎉 Example complete!");
    println!("💡 Key points:");
    println!("   • contract.balanceOf().call() → normal RPC (read-only)");
    println!("   • contract.transfer().send() → hijacked by relayer! (write)");
    println!("   • Your existing contract code works unchanged");
    println!("   • Just wrap your provider with with_relayer()");

    Ok(())
}

// Simulate what would happen with a real contract transaction
async fn simulate_contract_transaction() -> Result<String> {
    println!("📨 Relayer processing contract transaction...");

    // This represents what your relayer would do:
    // 1. Receive the transaction with contract call data
    // 2. Sign it using the relayer's private key
    // 3. Submit to the network
    // 4. Return transaction hash

    Ok("0x5678...contract-via-relayer".to_string())
}

// Mock provider for demo
#[derive(Clone)]
struct MockProvider;

impl MockProvider {
    fn new() -> Self {
        Self
    }
}

/// Helper: Create a relayer signer
fn create_relayer_signer() -> Result<RelayerSigner> {
    let relayer_id = RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?;
    let config = RelayerClientConfig {
        server_url: "https://api.relayer.example.com".to_string(),
        relayer_id: relayer_id.clone(),
        auth: RelayerClientAuth::ApiKey { api_key: "your-api-key-here".to_string() },
        speed: None,
    };
    let relayer_client = Arc::new(RelayerClient::new(config));

    Ok(RelayerSigner::new(
        relayer_id,
        relayer_client,
        Address::from_str("0x742d35cc6634c0532925a3b8d67e8000c942b1b5")?,
        Some(1), // Mainnet
    ))
}
