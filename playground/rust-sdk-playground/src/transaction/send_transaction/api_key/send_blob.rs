use anyhow::Result;
use rrelayer::{
    CreateRelayerClientConfig, EvmAddress, RelayTransactionRequest, RelayerClient, RelayerId,
    SendTransactionResult, TransactionData, TransactionSpeed, TransactionValue,
    create_relayer_client,
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

    let blob_data = vec![1u8; 131072]; // 128KB filled blob
    let hex_blob = format!("0x{}", alloy::hex::encode(&blob_data));

    let request = RelayTransactionRequest {
        to: EvmAddress::from_str("0x5FCD072a0BD58B6fa413031582E450FE724dba6D")?,
        value: TransactionValue::zero(),
        data: TransactionData::empty(),
        speed: Some(TransactionSpeed::FAST),
        external_id: None,
        blobs: Some(vec![hex_blob]),
    };

    let result: SendTransactionResult = relayer_client.transaction().send(&request, None).await?;
    println!("{:?}", result);

    let relay_transaction_result =
        relayer_client.transaction().wait_for_transaction_receipt_by_id(&result.id).await?;
    println!("{:?}", relay_transaction_result);

    Ok(())
}
