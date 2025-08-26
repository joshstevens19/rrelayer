use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate,
    commands::{
        error::RelayerManagementError, keystore::ProjectLocation, network::get_chain_id_for_network,
    },
};

/// Creates a new relayer for the specified network.
///
/// Authenticates the user, validates the network exists in the project configuration,
/// retrieves the chain ID for the network, and creates a new relayer with the given name.
/// Displays the created relayer details including ID, address, and network information.
///
/// # Arguments
/// * `name` - The name to assign to the new relayer
/// * `network` - The network name to create the relayer on
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Relayer created successfully
/// * `Err(RelayerManagementError)` - Authentication failed, network doesn't exist, or creation failed
pub async fn handle_create(
    name: &str,
    network: &str,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), RelayerManagementError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let setup_config = project_path.setup_config(false)?;
    if !setup_config.networks.iter().any(|n| n.name == network) {
        println!("Network {} does not exist", network);
        return Ok(());
    }

    let chain_id = get_chain_id_for_network(&network, project_path).await?;

    let result = sdk.relayer.create(chain_id, name).await?;

    println!("\n✅  Relayer created successfully!");
    println!("┌─────────────────────────────────────────────────");
    println!("│ Name:      {}", name);
    println!("│ ID:        {}", result.id);
    println!("│ Network:   {} (Chain ID: {})", network, chain_id);
    println!("│ Address:   {}", result.address);
    println!("└─────────────────────────────────────────────────");
    println!("\nUse 'relayer config get {}' to view more details.", result.id);

    Ok(())
}
