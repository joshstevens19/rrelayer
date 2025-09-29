//! Complete example showing RelayerSigner with all Alloy signing and transaction patterns

use alloy::{
    dyn_abi::{Eip712Domain, TypedData},
    network::TransactionBuilder,
    primitives::{Address, U256, keccak256},
    rpc::types::TransactionRequest,
    signers::Signer,
};
use eyre::Result;
use rrelayer::{
    RelayerClient, RelayerClientAuth, RelayerClientConfig, RelayerId, RelayerSigner, with_relayer,
};
use serde_json::json;
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Complete Alloy + Relayer Integration Example\n");

    // Create relayer signer
    let relayer_signer = create_relayer_signer()?;

    // 1. Alloy sign message
    example_1_sign_message(&relayer_signer).await?;

    // 2. Alloy sign typed data
    example_2_sign_typed_data(&relayer_signer).await?;

    // 3. Send transaction with Alloy
    example_3_send_transaction(&relayer_signer).await?;

    println!("\n🎉 All Alloy integration examples completed!");

    Ok(())
}

/// Example 1: Sign message using standard Alloy patterns
async fn example_1_sign_message(signer: &RelayerSigner) -> Result<()> {
    println!("✍️  Example 1: Alloy Message Signing");

    // Standard Alloy message signing patterns
    let message = b"Hello from Alloy + Relayer!";

    println!("📝 Signing message: '{}'", String::from_utf8_lossy(message));

    // Method 1: sign_message (most common Alloy pattern)
    match signer.sign_message(message).await {
        Ok(signature) => {
            println!("✅ Method 1 - signer.sign_message():");
            println!("   Signature: 0x{}", hex::encode(signature.as_bytes()));

            // Verify signature recovery
            match signature.recover_address_from_msg(message) {
                Ok(recovered_addr) => {
                    println!("   Recovered address: {}", recovered_addr);
                    println!("   Signer address: {}", signer.address());
                    if recovered_addr == *signer.address() {
                        println!("   ✅ Signature verification: PASSED");
                    } else {
                        println!("   ❌ Signature verification: FAILED");
                    }
                }
                Err(e) => println!("   ❌ Recovery failed: {}", e),
            }
        }
        Err(e) => {
            println!("❌ signer.sign_message() failed: {}", e);
            println!("   → Expected without valid relayer credentials");
        }
    }

    // Method 2: sign_hash (lower level)
    let message_hash = keccak256(message);
    println!("\n📝 Hash signing: 0x{}", hex::encode(message_hash));

    match signer.sign_hash(&message_hash).await {
        Ok(signature) => {
            println!("✅ Method 2 - signer.sign_hash():");
            println!("   Signature: 0x{}", hex::encode(signature.as_bytes()));
        }
        Err(e) => {
            println!("❌ signer.sign_hash() failed: {}", e);
            println!("   → Expected without valid relayer credentials");
        }
    }

    println!("   → Both methods route through relayer.sign().text()\n");
    Ok(())
}

/// Example 2: Sign typed data using Alloy EIP-712 patterns
async fn example_2_sign_typed_data(signer: &RelayerSigner) -> Result<()> {
    println!("📋 Example 2: Alloy EIP-712 Typed Data Signing");

    // Create EIP-712 domain (standard Alloy pattern)
    let _ = Eip712Domain {
        name: Some("AlloyRelayerExample".into()),
        version: Some("1".into()),
        chain_id: signer.chain_id().map(|id| U256::from(id)),
        verifying_contract: Some(Address::from_str("0x742d35cc6634c0532925a3b8d67e8000c942b1b5")?),
        salt: None,
    };

    // Create typed data JSON (EIP-712 standard)
    let typed_data_json = json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Mail": [
                {"name": "from", "type": "address"},
                {"name": "to", "type": "address"},
                {"name": "contents", "type": "string"}
            ]
        },
        "primaryType": "Mail",
        "domain": {
            "name": "AlloyRelayerExample",
            "version": "1",
            "chainId": signer.chain_id().unwrap_or(1),
            "verifyingContract": "0x742d35cc6634c0532925a3b8d67e8000c942b1b5"
        },
        "message": {
            "from": "0x742d35cc6634c0532925a3b8d67e8000c942b1b5",
            "to": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
            "contents": "Hello EIP-712 via Relayer!"
        }
    });

    println!("📋 EIP-712 Typed Data:");
    println!("   Domain: AlloyRelayerExample v1");
    println!("   Type: Mail");
    println!("   Message: Hello EIP-712 via Relayer!");

    // Convert to Alloy TypedData
    let typed_data: TypedData = serde_json::from_value(typed_data_json)?;

    // Sign using Alloy's typed data signing
    match signer.sign_dynamic_typed_data(&typed_data).await {
        Ok(signature) => {
            println!("✅ signer.sign_dynamic_typed_data():");
            println!("   Signature: 0x{}", hex::encode(signature.as_bytes()));
            println!("   → Routed through relayer.sign().typed_data()");
        }
        Err(e) => {
            println!("❌ Typed data signing failed: {}", e);
            println!("   → Expected without valid relayer credentials");
        }
    }

    println!();
    Ok(())
}

/// Example 3: Send transaction using Alloy patterns with relayer hijacking
async fn example_3_send_transaction(relayer_signer: &RelayerSigner) -> Result<()> {
    println!("💸 Example 3: Alloy Transaction Sending (Hijacked)");

    // Standard Alloy transaction building
    let from = relayer_signer.address();
    let to = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
    let value = U256::from(1000000000000000000u64); // 1 ETH

    let tx_request = TransactionRequest::default()
        .with_from(*from)
        .with_to(to)
        .with_value(value)
        .with_gas_limit(21000)
        .with_gas_price(20000000000u128); // 20 gwei

    println!("💰 Transaction Details:");
    println!("   From: {}", from);
    println!("   To: {:?}", tx_request.to);
    println!("   Value: {} ETH", U256::from(value) / U256::from(10u64.pow(18)));
    println!("   Gas: {:?}", tx_request.gas);
    println!("   Gas Price: {:?} gwei", tx_request.gas_price.map(|p| p / 1000000000));

    // Create provider with relayer hijacking
    let mock_provider = (); // In practice: real HTTP provider
    let hijacked_provider = with_relayer(mock_provider, relayer_signer.clone());

    println!("\n🔄 Method 1: hijacked_provider.send_transaction_via_relayer()");

    // Method 1: Direct relayer transaction (our custom method)
    match hijacked_provider.send_transaction_via_relayer(to, 1000000000000000000u64).await {
        Ok(tx_id) => {
            println!("✅ Transaction sent via relayer!");
            println!("   Transaction ID: {}", tx_id);
            println!("   → Used relayer.transaction().send()");
        }
        Err(e) => {
            println!("❌ Transaction failed: {}", e);
            println!("   → Expected without valid relayer credentials");
        }
    }

    // Method 2: Show how normal Alloy provider.send_transaction() would be hijacked
    println!("\n🔄 Method 2: Standard Alloy pattern (would be hijacked)");
    println!("   Code: provider.send_transaction(tx_request).await");
    println!("   → This would be intercepted by RelayerProvider");
    println!("   → Converted from TransactionRequest to RelayTransactionRequest");
    println!("   → Sent via relayer.transaction().send()");
    println!("   → Returns as if from normal provider");

    // Method 3: Contract interaction example
    println!("\n🔄 Method 3: Contract interaction (would be hijacked)");
    println!("   Code: contract.transfer(recipient, amount).send().await");
    println!("   → Contract calls get converted to transaction data");
    println!("   → RelayerProvider intercepts the .send() call");
    println!("   → Routes through relayer with contract call data");

    println!("\n💡 Key Benefits:");
    println!("   ✅ Use standard Alloy TransactionRequest");
    println!("   ✅ All signing goes through RelayerSigner automatically");
    println!("   ✅ All transactions get hijacked to use relayer");
    println!("   ✅ Existing Alloy code works unchanged!");

    Ok(())
}

/// Helper: Create a relayer signer for examples
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
        Some(11155111), // Sepolia testnet
    ))
}
