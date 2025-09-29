use anyhow::Result;
use rrelayer::{
    Client, CreateClientAuth, CreateClientConfig, CreateRelayerResult, RelayerId, TransactionSpeed,
    create_client,
};
use std::str::FromStr;

async fn get_client() -> Result<Client> {
    let client = create_client(CreateClientConfig {
        server_url: "http://localhost:8000".to_string(),
        auth: CreateClientAuth {
            username: "your_username".to_string(),
            password: "your_password".to_string(),
        },
    });

    Ok(client)
}

async fn example() -> Result<()> {
    let client = get_client().await?;

    let result: CreateRelayerResult = client
        .relayer()
        .clone_relayer(
            &RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
            11155111,
            "cloned-relayer",
        )
        .await?;
    println!("{:?}", result);

    let relayer_client =
        client.get_relayer_client(&result.id, Some(TransactionSpeed::FAST)).await?;

    Ok(())
}
