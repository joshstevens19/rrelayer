use alloy::sol;
use alloy::sol_types::SolCall;
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

sol! {
    interface IERC20 {
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

async fn example() -> Result<()> {
    let relayer_client = get_relayer_client().await?;

    let transfer_call = IERC20::transferCall {
        to: EvmAddress::from_str("0x742d35Cc82C8c6c0dA18Affe2b08BF36C4b0E5Dc")?.into_address(),
        amount: TransactionValue::from_str("1000000000000000000")
            .map_err(|e| anyhow::anyhow!("Invalid value: {}", e))?
            .into_inner(),
    };

    let encoded_data = transfer_call.abi_encode();

    let request = RelayTransactionRequest {
        to: EvmAddress::from_str("0x5FCD072a0BD58B6fa413031582E450FE724dba6D")?,
        value: TransactionValue::zero(),
        data: TransactionData::from(encoded_data),
        speed: Some(TransactionSpeed::FAST),
        external_id: None,
        blobs: None,
    };

    let result: SendTransactionResult = relayer_client.transaction().send(&request, None).await?;
    println!("{:?}", result);

    let relay_transaction_result =
        relayer_client.transaction().wait_for_transaction_receipt_by_id(&result.id).await?;
    println!("{:?}", relay_transaction_result);

    Ok(())
}
