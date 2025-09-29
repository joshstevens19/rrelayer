use anyhow::{Context, Result};
use rrelayer::{
    CreateRelayerClientConfig, RelayerClient, RelayerId, SignTypedDataResult, TransactionSpeed,
    TypedData, create_relayer_client,
};
use std::str::FromStr;

async fn get_relayer_client() -> Result<RelayerClient> {
    let relayer: RelayerClient = create_relayer_client(CreateRelayerClientConfig {
        server_url: "http://localhost:8000".to_string(),
        relayer_id: RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
        api_key: "YOUR_API_KEY".to_string(),
        speed: Some(TransactionSpeed::FAST),
    });

    Ok(relayer)
}

async fn example() -> Result<()> {
    let relayer_client = get_relayer_client().await?;

    let chain_id = relayer_client.get_info().await?.chain_id;

    let typed_data_json = serde_json::json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Mail": [
                {"name": "from", "type": "Person"},
                {"name": "to", "type": "Person"},
                {"name": "contents", "type": "string"}
            ],
            "Person": [
                {"name": "name", "type": "string"},
                {"name": "wallet", "type": "address"}
            ]
        },
        "primaryType": "Mail",
        "domain": {
            "name": "RRelayer Test",
            "version": "1",
            "chainId": chain_id,
            "verifyingContract": "0x0000000000000000000000000000000000000000"
        },
        "message": {
            "from": {
                "name": "Alice",
                "wallet": "0x1234567890123456789012345678901234567890"
            },
            "to": {
                "name": "Bob",
                "wallet": "0x0987654321098765432109876543210987654321"
            },
            "contents": "Hello from E2E test!"
        }
    });

    let typed_data: TypedData =
        serde_json::from_value(typed_data_json).context("Failed to create typed data")?;

    let result: SignTypedDataResult = relayer_client.sign().typed_data(&typed_data, None).await?;
    println!("{:?}", result);

    Ok(())
}
