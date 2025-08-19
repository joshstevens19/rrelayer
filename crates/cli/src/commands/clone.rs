use rrelayer_core::relayer::types::RelayerId;
use rrelayer_sdk::SDK;

use crate::commands::network::get_chain_id_for_network;
use crate::{authentication::handle_authenticate, commands::keystore::ProjectLocation};

pub async fn handle_clone(
    relayer_id: &RelayerId,
    name: &str,
    network: &str,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let setup_config = project_path.setup_config(false)?;
    if !setup_config.networks.iter().any(|n| n.name == network) {
        println!("Network {} does not exist", network);
        return Ok(());
    }

    let chain_id = get_chain_id_for_network(&network, project_path).await?;

    let result = sdk.relayer.clone(relayer_id, chain_id, name).await?;

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
