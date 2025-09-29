use anyhow::Result;
use rrelayer::{Client, CreateClientAuth, CreateClientConfig, Network, create_client};

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

    let network: Option<Network> = client.network().get(11155111).await?;
    println!("{:?}", network);

    Ok(())
}
