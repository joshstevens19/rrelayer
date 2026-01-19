//! Example of how to sign messages using RelayerSigner - routes through relayer!

use alloy::{primitives::Address, signers::Signer};
use eyre::Result;
use rrelayer::{RelayerClient, RelayerClientAuth, RelayerClientConfig, RelayerId, RelayerSigner};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    println!("✍️  Message Signing via Relayer Example");

    // Create a RelayerSigner (like PrivateKeySigner::random() but for relayer)
    let signer = create_relayer_signer()?;

    // Optionally set chain ID for EIP-155 replay protection
    let signer = signer.with_chain_id(Some(1337));

    println!("✅ Created RelayerSigner: {}", signer.address());
    println!("   Chain ID: {:?}", signer.chain_id());

    // Message to sign (exactly like the original Alloy example)
    let message = b"hello from relayer";

    println!("\n📝 Signing message: '{}'", String::from_utf8_lossy(message));
    println!("   Using: signer.sign_message(message)");

    // Sign the message - this routes through relayer.sign().text()
    match signer.sign_message(message).await {
        Ok(signature) => {
            println!("✅ Message signed via relayer!");
            println!("   Signature: {:?}", signature);
            println!("   → This signature came from your relayer service!");

            // Verify signature (exactly like original Alloy example)
            match signature.recover_address_from_msg(&message[..]) {
                Ok(recovered) => {
                    println!("\n🔍 Signature verification:");
                    println!("   Signer address: {}", signer.address());
                    println!("   Recovered address: {}", recovered);

                    if recovered == *signer.address() {
                        println!("✅ Signature verification successful!");
                    } else {
                        println!("❌ Signature verification failed!");
                    }
                }
                Err(e) => println!("❌ Could not recover address: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Failed to sign: {}", e);
            println!("   → This is expected without valid relayer credentials");
            println!("   → Would work with real relayer setup");
        }
    }

    println!("\n🎉 Example complete!");
    println!("💡 RelayerSigner works exactly like PrivateKeySigner!");
    println!("   • Same Signer trait implementation");
    println!("   • Same method signatures");
    println!("   • But routes through your relayer service");

    Ok(())
}

/// Helper: Create a relayer signer
fn create_relayer_signer() -> Result<RelayerSigner> {
    let relayer_id = RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?;
    let config = RelayerClientConfig {
        server_url: "https://api.relayer.example.com".to_string(),
        relayer_id,
        auth: RelayerClientAuth::ApiKey { api_key: "your-api-key-here".to_string() },
        fallback_speed: None,
    };
    let relayer_client = Arc::new(RelayerClient::new(config));

    Ok(RelayerSigner::from_relayer_client(
        relayer_client,
        Address::from_str("0x742d35cc6634c0532925a3b8d67e8000c942b1b5")?,
        Some(1), // Mainnet
    ))
}
