use rrelayer_core::relayer::RelayerId;
use rrelayer::Client;

use crate::commands::error::RelayerManagementError;
use crate::commands::network::get_chain_id_for_network;
use crate::project_location::ProjectLocation;

pub async fn handle_clone(
    relayer_id: &RelayerId,
    name: &str,
    network: &str,
    project_path: &ProjectLocation,
    client: &Client,
) -> Result<(), RelayerManagementError> {
    let setup_config = project_path.setup_config(false)?;
    if !setup_config.networks.iter().any(|n| n.name == network) {
        println!("Network {} does not exist", network);
        return Ok(());
    }

    let client = client.get_relayer_client(relayer_id, None).await?;

    let chain_id = get_chain_id_for_network(&network, project_path).await?;

    let result = client.clone_relayer(&chain_id, name).await?;

    println!("\n✅  Relayer cloned successfully!");
    println!("┌─────────────────────────────────────────────────");
    println!("│ Name:      {}", name);
    println!("│ ID:        {}", result.id);
    println!("│ Network:   {} (Chain ID: {})", network, chain_id);
    println!("│ Address:   {}", result.address);
    println!("└─────────────────────────────────────────────────");
    println!("\nUse 'relayer config get {}' to view more details.", result.id);

    Ok(())
}
