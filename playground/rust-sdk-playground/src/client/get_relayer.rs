use rrelayer::{
    CreateRelayerClientConfig, RelayerClient, RelayerId, TransactionSpeed, create_relayer_client,
};
use std::str::FromStr;

pub async fn get_relayer() -> anyhow::Result<RelayerClient> {
    let relayer: RelayerClient = create_relayer_client(CreateRelayerClientConfig {
        server_url: "http://localhost:8000".to_string(),
        relayer_id: RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
        api_key: "YOUR_API_KEY".to_string(),
        speed: Some(TransactionSpeed::FAST),
    });

    Ok(relayer)
}
