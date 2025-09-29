use anyhow::Result;
use rrelayer::{
    CreateRelayerClientConfig, EvmAddress, RelayTransactionRequest, RelayerClient, RelayerId,
    TransactionSpeed, TransactionValue, create_relayer_client,
};
use std::str::FromStr;

async fn get_relayer() -> Result<RelayerClient> {
    let relayer: RelayerClient = create_relayer_client(CreateRelayerClientConfig {
        server_url: "http://localhost:8000".to_string(),
        relayer_id: RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
        api_key: "YOUR_API_KEY".to_string(),
        speed: Some(TransactionSpeed::FAST),
    });

    Ok(relayer)
}

async fn rate_limit_example() -> Result<()> {
    let relayer = get_relayer().await?;

    let tx_request = RelayTransactionRequest {
        to: EvmAddress::from_str("0xa4635F69E5A64CD48da3FbC999aCed87B00756F6")?,
        value: TransactionValue::from_str("1000000000000000000")
            .map_err(|e| anyhow::anyhow!("Invalid value: {}", e))?, // 1 ETH in wei
        data: Default::default(),
        speed: None,
        external_id: None,
        blobs: None,
    };

    // Passing in the user doing the transaction as an example, this rate limit key will be limited based on what is configured
    relayer
        .transaction()
        .send(&tx_request, Some("user__0x5FCD072a0BD58B6fa413031582E450FE724dba6D".to_string()))
        .await?;

    Ok(())
}
