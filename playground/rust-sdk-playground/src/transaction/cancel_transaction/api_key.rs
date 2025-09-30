use anyhow::Result;
use rrelayer::{
    CancelTransactionResponse, CreateRelayerClientConfig, RelayerClient, RelayerId, TransactionId,
    TransactionSpeed, create_relayer_client,
};
use std::str::FromStr;

async fn get_relayer_client() -> Result<RelayerClient> {
    let relayer: RelayerClient = create_relayer_client(CreateRelayerClientConfig {
        server_url: "http://localhost:8000".to_string(),
        relayer_id: RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?,
        api_key: "YOUR_API_KEY".to_string(),
        fallback_speed: Some(TransactionSpeed::FAST),
    });

    Ok(relayer)
}

async fn example() -> Result<()> {
    let relayer_client = get_relayer_client().await?;

    let result: CancelTransactionResponse = relayer_client
        .transaction()
        .cancel(
            &TransactionId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")
                .map_err(|e| anyhow::anyhow!("Invalid tx id: {}", e))?,
            None,
        )
        .await?;
    println!("{:?}", result);

    if result.success
        && let Some(tx_id) = result.cancel_transaction_id
    {
        let relay_transaction_result =
            relayer_client.transaction().wait_for_transaction_receipt_by_id(&tx_id).await?;
        println!("{:?}", relay_transaction_result);

        Ok(())
    } else {
        Err(anyhow::anyhow!("Could not cancel transaction"))
    }
}
