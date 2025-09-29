use anyhow::Result;
use rrelayer::{
    Client, CreateClientAuth, CreateClientConfig, CreateRelayerResult, TransactionSpeed,
    create_client,
};

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

    let result: CreateRelayerResult = client.relayer().create(11155111, "fancy_relayer").await?;
    println!("{:?}", result);

    let relayer_client =
        client.get_relayer_client(&result.id, Some(TransactionSpeed::FAST)).await?;

    Ok(())
}
