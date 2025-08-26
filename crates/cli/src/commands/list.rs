use rrelayer_core::common_types::PagingContext;
use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate,
    commands::{
        error::RelayerManagementError, keystore::ProjectLocation, network::get_chain_id_for_network,
    },
    console::print_table,
};

/// Lists relayers, optionally filtered by network.
///
/// Authenticates the user and retrieves all relayers. If a network is specified,
/// only relayers for that network's chain ID are returned. Displays the results
/// in a formatted table with relayer details.
///
/// # Arguments
/// * `network` - Optional network name to filter relayers by
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Relayers listed successfully
/// * `Err(RelayerManagementError)` - Authentication failed or API call failed
pub async fn handle_list(
    network: &Option<String>,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), RelayerManagementError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    if let Some(network) = network {
        let chain_id = get_chain_id_for_network(&network, project_path).await?;
        log_relayers(sdk, Some(chain_id)).await?;
        return Ok(());
    } else {
        log_relayers(sdk, None).await?;
    }

    Ok(())
}

/// Retrieves and displays relayers in a formatted table.
///
/// Fetches relayers from the API, optionally filtered by chain ID, and displays
/// them in a table format with columns for ID, name, chain ID, address, max gas price,
/// status, allowlisted status, and EIP-1559 enablement.
///
/// # Arguments
/// * `sdk` - Mutable reference to the SDK for making API calls
/// * `chain_id` - Optional chain ID to filter relayers by
///
/// # Returns
/// * `Ok(())` - Relayers displayed successfully
/// * `Err(RelayerManagementError)` - Failed to fetch relayers from API
async fn log_relayers(sdk: &mut SDK, chain_id: Option<u64>) -> Result<(), RelayerManagementError> {
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
