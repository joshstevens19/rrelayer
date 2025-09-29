use anyhow::Result;
use rrelayer::{Client, CreateClientAuth, CreateClientConfig, RelayerId, create_client};
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

    client.relayer().delete(&RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?).await?;

    Ok(())
}
