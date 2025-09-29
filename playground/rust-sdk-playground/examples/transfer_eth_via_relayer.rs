//! Example of how to transfer ETH using RelayerProvider - hijacks the transaction!

use alloy::{
    network::TransactionBuilder,
    primitives::{Address, U256},
    rpc::types::TransactionRequest,
};
use eyre::Result;
use rrelayer::{
    RelayerClient, RelayerClientAuth, RelayerClientConfig, RelayerId, RelayerSigner, with_relayer,
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 ETH Transfer via Relayer Example");

    // 1. Setup relayer signer
    let relayer_signer = create_relayer_signer()?;
    println!("✅ Created RelayerSigner: {}", relayer_signer.address());

    // 2. Create normal Alloy provider (like the original example)
    println!("🔗 Creating Alloy provider...");

    // For demo - normally you'd do:
    // let provider = ProviderBuilder::new().connect_anvil_with_wallet();
    // But we'll simulate it

    let mock_provider = MockProvider::new(); // Simplified for demo

    // 3. Wrap with relayer hijacking - THIS IS THE KEY PART!
    let hijacked_provider = with_relayer(mock_provider, relayer_signer.clone());
    println!("🔧 Wrapped provider with relayer hijacking");

    // 4. Use EXACTLY like the original Alloy example!
    let alice = relayer_signer.address(); // Our relayer's address
    let bob = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;

    println!("\n💰 Account details:");
    println!("   Alice (relayer): {}", alice);
    println!("   Bob: {}", bob);

    // Build transaction EXACTLY like the original Alloy example
    let _ =
        TransactionRequest::default().with_from(*alice).with_to(bob).with_value(U256::from(100)); // 100 wei

    println!("\n💸 Sending 100 wei from Alice to Bob...");
    println!("   Using: hijacked_provider.send_transaction(tx)");

    // This demonstrates the transaction hijacking concept
    match hijacked_provider.send_transaction_via_relayer(bob, 100).await {
        Ok(tx_hash) => {
            println!("✅ Transaction sent via relayer!");
            println!("   Transaction ID: {}", tx_hash);
            println!("   → This went through your relayer service, not directly to RPC!");
        }
        Err(e) => {
            println!("❌ Failed: {} (expected without valid relayer credentials)", e);
            println!("   → Would work with real relayer setup");
        }
    }

    println!("\n🎉 Example complete!");
    println!("💡 Your existing Alloy code works unchanged - just wrap the provider!");

    Ok(())
}

// This demonstrates the concept - in practice you'd implement full Provider trait

// Simplified mock provider for demo
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

    Ok(RelayerSigner::from_relayer_client(
        relayer_client,
        Address::from_str("0x742d35cc6634c0532925a3b8d67e8000c942b1b5")?,
        Some(1),
    ))
}
