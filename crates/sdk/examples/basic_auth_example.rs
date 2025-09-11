use rrelayer_sdk::SDK;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create SDK with basic authentication
    let sdk = SDK::new(
        "http://localhost:8080".to_string(),
        "admin".to_string(),         // Username from RRELAYER_AUTH_USERNAME
        "your_password".to_string(), // Password from RRELAYER_AUTH_PASSWORD
    );

    // Test authentication
    match sdk.test_auth().await {
        Ok(_) => println!("✅ Basic authentication successful!"),
        Err(e) => println!("❌ Authentication failed: {}", e),
    }

    // Example API calls (all automatically use basic auth)

    // Check server health (no auth required)
    match sdk.health.check().await {
        Ok(_) => println!("✅ Server health check passed"),
        Err(e) => println!("❌ Health check failed: {}", e),
    }

    // Get gas prices (basic auth required)
    match sdk.gas.get_gas_price(1).await {
        // Chain ID 1 = Ethereum
        Ok(gas_prices) => println!("✅ Gas prices: {:?}", gas_prices),
        Err(e) => println!("❌ Failed to get gas prices: {}", e),
    }

    // Get networks (basic auth required)
    match sdk.network.get_networks().await {
        Ok(networks) => println!("✅ Found {} networks", networks.len()),
        Err(e) => println!("❌ Failed to get networks: {}", e),
    }

    Ok(())
}
