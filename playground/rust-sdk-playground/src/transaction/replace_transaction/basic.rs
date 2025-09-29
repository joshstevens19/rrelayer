use anyhow::Result;
use rrelayer::{
    AdminRelayerClient, CreateClientAuth, CreateClientConfig, EvmAddress, RelayTransactionRequest,
    RelayerId, ReplaceTransactionResult, TransactionData, TransactionId, TransactionSpeed,
    TransactionValue, create_client,
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

    let request = RelayTransactionRequest {
        to: EvmAddress::from_str("0x5FCD072a0BD58B6fa413031582E450FE724dba6D")?,
        value: TransactionValue::from_str("1000000000000000000")
            .map_err(|e| anyhow::anyhow!("Invalid value: {}", e))?, // 1 ETH in wei
        data: TransactionData::empty(),
        speed: Some(TransactionSpeed::FAST),
        external_id: None,
        blobs: None,
    };

    let result: ReplaceTransactionResult = relayer_client
        .transaction()
        .replace(
            &TransactionId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")
                .map_err(|e| anyhow::anyhow!("Invalid tx id: {}", e))?,
            &request,
            None,
        )
        .await?;
    println!("{:?}", result);

    if result.success
        && let Some(tx_id) = result.replace_transaction_id
    {
        let relay_transaction_result =
            relayer_client.transaction().wait_for_transaction_receipt_by_id(&tx_id).await?;
        println!("{:?}", relay_transaction_result);

        Ok(())
    } else {
        Err(anyhow::anyhow!("Could not replace transaction"))
    }
}
