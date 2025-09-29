use rrelayer::{Client, CreateClientAuth, CreateClientConfig, create_client};

pub async fn get_client() -> anyhow::Result<Client> {
    let client = create_client(CreateClientConfig {
        server_url: "http://localhost:8000".to_string(),
        auth: CreateClientAuth {
            username: "your_username".to_string(),
            password: "your_password".to_string(),
        },
    });

    Ok(client)
}
