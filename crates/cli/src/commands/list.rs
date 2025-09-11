use rrelayer_core::common_types::PagingContext;
use rrelayer_sdk::SDK;

use crate::project_location::ProjectLocation;
use crate::{
    commands::{error::RelayerManagementError, network::get_chain_id_for_network},
    console::print_table,
};

pub async fn handle_list(
    network: &Option<String>,
    project_path: &ProjectLocation,
    sdk: &SDK,
) -> Result<(), RelayerManagementError> {
    if let Some(network) = network {
        let chain_id = get_chain_id_for_network(&network, project_path).await?;
        log_relayers(sdk, Some(chain_id)).await?;
        return Ok(());
    } else {
        log_relayers(sdk, None).await?;
    }

    Ok(())
}

async fn log_relayers(sdk: &SDK, chain_id: Option<u64>) -> Result<(), RelayerManagementError> {
    let relayers = sdk
        .relayer
        .get_all(
            chain_id,
            &PagingContext {
                // TODO: handle paging later
                limit: 1000,
                offset: 0,
            },
        )
        .await?
        .items;

    if relayers.is_empty() {
        println!("No relayers found.");
        return Ok(());
    }

    let mut rows = Vec::new();
    for relayer in relayers.iter() {
        let max_gas = match &relayer.max_gas_price {
            Some(price) => format!("{}", price.into_u128()),
            None => "None".to_string(),
        };

        let status = if relayer.paused { "Paused" } else { "Active" };

        rows.push(vec![
            relayer.id.to_string(),
            relayer.name.clone(),
            relayer.chain_id.to_string(),
            relayer.address.hex(),
            max_gas,
            status.to_string(),
            format!("{}", relayer.allowlisted_only),
            format!("{}", relayer.eip_1559_enabled),
        ]);
    }

    let headers = vec![
        "Id",
        "Name",
        "Chain ID",
        "Address",
        "Max Gas Price",
        "Status",
        "Allowlisted Only",
        "EIP-1559 Enabled",
    ];

    let title = format!("{} Relayers:", relayers.len());
    print_table(headers, rows, Some(&title), None);

    Ok(())
}
