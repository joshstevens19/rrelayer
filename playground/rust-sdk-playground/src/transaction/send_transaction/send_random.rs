use anyhow::Result;
use rrelayer::{
    Client, CreateClientAuth, CreateClientConfig, CreateRelayerClientConfig, EvmAddress,
    RelayTransactionRequest, RelayerClient, RelayerId, SendTransactionResult, TransactionData,
    TransactionSpeed, TransactionValue, create_client, create_relayer_client,
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

    let request = RelayTransactionRequest {
        to: EvmAddress::from_str("0x5FCD072a0BD58B6fa413031582E450FE724dba6D")?,
        value: TransactionValue::from_str("1000000000000000000")
            .map_err(|e| anyhow::anyhow!("Invalid value: {}", e))?, // 1 ETH in wei
        data: TransactionData::empty(),
        speed: Some(TransactionSpeed::FAST),
        external_id: None,
        blobs: None,
    };

    let transaction = client.transaction();

    // Send to a random relayer on the specified chain
    let result: SendTransactionResult = transaction.send_random(1, &request, None).await?;
    println!("Contract transaction sent via random relayer: {:?}", result);

    let relay_transaction_result =
        transaction.wait_for_transaction_receipt_by_id(&result.id).await?;
    println!("Transaction confirmed: {:?}", relay_transaction_result);

    Ok(())
}
