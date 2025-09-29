use anyhow::Result;
use rrelayer::{
    AdminRelayerClient, CreateClientAuth, CreateClientConfig, RelayerId, TransactionCountType,
    TransactionSpeed, create_client,
};
use std::str::FromStr;

async fn get_relayer_client() -> Result<AdminRelayerClient> {
    let client = create_client(CreateClientConfig {
        server_url: "http://localhost:8000".to_string(),
        auth: CreateClientAuth {
            username: "your_username".to_string(),
            password: "your_password".to_string(),
        },
    });

    let relayer_client: AdminRelayerClient = client
        .get_relayer_client(
            &RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
            Some(TransactionSpeed::FAST),
        )
        .await?;

    Ok(relayer_client)
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
