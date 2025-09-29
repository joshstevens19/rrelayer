use anyhow::Result;
use rrelayer::{
    create_relayer_client,
    CreateRelayerClientConfig, RelayerClient, RelayerId, TransactionCountType,
    TransactionSpeed,
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

    let pending_count: u32 =
        relayer_client.transaction().get_count(TransactionCountType::Pending).await?;
    println!("{:?}", pending_count);

    let inmempool_count: u32 =
        relayer_client.transaction().get_count(TransactionCountType::Pending).await?;
    println!("{:?}", inmempool_count);

    Ok(())
}
