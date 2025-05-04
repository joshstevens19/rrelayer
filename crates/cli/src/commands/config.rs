use clap::Subcommand;
use rrelayerr_core::relayer::types::RelayerId;
use rrelayerr_sdk::SDK;

use crate::{authentication::handle_authenticate, commands::keystore::ProjectLocation};

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get detailed information about the relayer
    Get {
        /// The unique identifier of the relayer
        relayer_id: RelayerId,
    },
    /// Pause operations for a specific relayer
    Pause {
        /// The unique identifier of the relayer
        relayer_id: RelayerId,
    },
    /// Resume operations for a paused relayer
    Unpause {
        /// The unique identifier of the relayer
        relayer_id: RelayerId,
    },
    /// Configure EIP1559 transaction support for a relayer
    UpdateEip1559Status {
        /// The unique identifier of the relayer
        relayer_id: RelayerId,
        /// Enable or disable EIP1559 support
        status: bool,
    },
    /// Set the maximum gas price limit for a relayer
    UpdateMaxGasPrice {
        /// The unique identifier of the relayer
        relayer_id: RelayerId,
        /// Maximum gas price in wei
        cap: u64,
    },
}

pub async fn handle_config(
    command: &ConfigCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ConfigCommand::Get { relayer_id } => handle_get(relayer_id, project_path, sdk).await,
        ConfigCommand::Pause { relayer_id } => handle_pause(relayer_id),
        ConfigCommand::Unpause { relayer_id } => handle_unpause(relayer_id),
        ConfigCommand::UpdateEip1559Status { relayer_id, status } => {
            handle_update_eip1559_status(relayer_id, *status)
        }
        ConfigCommand::UpdateMaxGasPrice { relayer_id, cap } => handle_update_max_gas_price(relayer_id, *cap),
    }
}

pub async fn handle_get(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let result = sdk.relayer.get(&relayer_id).await?;
    match result {
        Some(result) => {
            let relayer = &result.relayer;
            let provider_urls = &result.provider_urls;

            let created_at = chrono::DateTime::<chrono::Utc>::from(relayer.created_at)
                .format("%Y-%m-%d %H:%M:%S UTC");

            let max_gas = match &relayer.max_gas_price {
                Some(price) => price.into_u128().to_string(),
                None => "Not set".to_string(),
            };

            let status = if relayer.paused { "Paused" } else { "Active" };

            println!("┌─────────────────────────────────────────────────────────────────────");
            println!("│ RELAYER DETAILS");
            println!("├─────────────────────────────────────────────────────────────────────");
            println!("│ ID:                  {}", relayer.id);
            println!("│ Name:                {}", relayer.name);
            println!("│ Chain ID:            {}", relayer.chain_id);
            println!("│ Address:             {}", relayer.address.hex());
            println!("│ Status:              {}", status);
            println!("│ Max Gas Price:       {}", max_gas);
            println!("│ Wallet Index:        {}", relayer.wallet_index);
            println!("│ Allowlisted Only:    {}", relayer.allowlisted_only);
            println!("│ EIP-1559 Enabled:    {}", relayer.eip_1559_enabled);
            println!("│ Created At:          {}", created_at);

            println!("├─────────────────────────────────────────────────────────────────────");
            println!("│ PROVIDER URLS");
            println!("├─────────────────────────────────────────────────────────────────────");

            if provider_urls.is_empty() {
                println!("│ No provider URLs configured.");
            } else {
                for (i, url) in provider_urls.iter().enumerate() {
                    println!("│ {:<2}: {}", i + 1, url);
                }
            }
            println!("└─────────────────────────────────────────────────────────────────────");
        }
        None => {
            println!("Relayer {} not found.", relayer_id);
        }
    }

    Ok(())
}

fn handle_pause(relayer_id: &RelayerId) -> Result<(), Box<dyn std::error::Error>> {
    println!("Pausing relayer with ID: {}", relayer_id);
    // TODO: Implement actual relayer pausing logic
    Ok(())
}

fn handle_unpause(relayer_id: &RelayerId) -> Result<(), Box<dyn std::error::Error>> {
    println!("Unpausing relayer with ID: {}", relayer_id);
    // TODO: Implement actual relayer unpausing logic
    Ok(())
}

fn handle_update_eip1559_status(
    relayer_id: &RelayerId,
    status: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Updating EIP1559 status for relayer {} to: {}", relayer_id, status);
    // TODO: Implement actual EIP1559 status update logic
    Ok(())
}

fn handle_update_max_gas_price(
    relayer_id: &RelayerId,
    cap: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Updating max gas price for relayer {} to: {}", relayer_id, cap);
    // TODO: Implement actual max gas price update logic
    Ok(())
}
