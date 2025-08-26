use clap::Subcommand;
use rrelayer_core::relayer::types::RelayerId;
use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::error::ConfigError,
    commands::keystore::ProjectLocation,
};

#[derive(Subcommand)]
pub enum GasCommand {
    /// Add an address to allowlist
    MaxPrice {
        /// Maximum gas price in wei
        #[clap(required = true)]
        cap: u128,
    },
    /// Enable legacy transactions gas support which will be none 1559
    Legacy,
    /// Enable the latest gas standard for transactions which is 1559
    Latest,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get detailed information about the relayer
    Get {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// Pause operations for a specific relayer
    Pause {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// Resume operations for a paused relayer
    Unpause {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    Gas {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        #[command(subcommand)]
        command: GasCommand,
    },
}

/// Handles relayer configuration command routing and execution.
///
/// Routes configuration commands to appropriate handlers for getting relayer details,
/// pausing/unpausing operations, and managing gas settings.
///
/// # Arguments
/// * `command` - The configuration command to execute
/// * `project_path` - Project location for authentication
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(ConfigError)` - Command execution failed
pub async fn handle_config(
    command: &ConfigCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
    match command {
        ConfigCommand::Get { relayer_id } => handle_get(relayer_id, project_path, sdk).await,
        ConfigCommand::Pause { relayer_id } => handle_pause(relayer_id, project_path, sdk).await,
        ConfigCommand::Unpause { relayer_id } => {
            handle_unpause(relayer_id, project_path, sdk).await
        }
        ConfigCommand::Gas { relayer_id, command } => match command {
            GasCommand::MaxPrice { cap } => {
                handle_update_max_gas_price(relayer_id, *cap, project_path, sdk).await
            }
            GasCommand::Legacy => {
                handle_update_eip1559_status(relayer_id, false, project_path, sdk).await
            }
            GasCommand::Latest => {
                handle_update_eip1559_status(relayer_id, true, project_path, sdk).await
            }
        },
    }
}

/// Retrieves and displays detailed information about a relayer.
///
/// Shows comprehensive relayer details including configuration, status,
/// gas settings, provider URLs, and operational parameters.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer
/// * `project_path` - Project location for authentication
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - Relayer information displayed successfully
/// * `Err(ConfigError)` - Operation failed
pub async fn handle_get(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
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

/// Handles the pause command to temporarily disable relayer operations.
///
/// Pauses the specified relayer, preventing it from processing new transactions
/// while maintaining its configuration and state for later resumption.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer to pause
/// * `project_path` - Project location containing configuration
/// * `sdk` - SDK instance for API communication
///
/// # Returns
/// * `Ok(())` - Relayer paused successfully
/// * `Err(ConfigError)` - Pause operation failed
async fn handle_pause(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.pause(&relayer_id).await?;

    println!("Paused relayer {}", relayer_id);

    Ok(())
}

/// Handles the unpause command to resume relayer operations.
///
/// Resumes operations for a previously paused relayer, allowing it to continue
/// processing transactions from where it left off.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer to resume
/// * `project_path` - Project location containing configuration
/// * `sdk` - SDK instance for API communication
///
/// # Returns
/// * `Ok(())` - Relayer resumed successfully
/// * `Err(ConfigError)` - Unpause operation failed
async fn handle_unpause(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.unpause(&relayer_id).await?;

    println!("Unpaused relayer {}", relayer_id);

    Ok(())
}

/// Handles updating the EIP-1559 transaction status for a relayer.
///
/// Enables or disables EIP-1559 transaction support for the specified relayer,
/// allowing control over whether to use modern gas pricing or legacy transactions.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer to update
/// * `status` - True to enable EIP-1559, false to use legacy transactions
/// * `project_path` - Project location containing configuration
/// * `sdk` - SDK instance for API communication
///
/// # Returns
/// * `Ok(())` - EIP-1559 status updated successfully
/// * `Err(ConfigError)` - Update operation failed
async fn handle_update_eip1559_status(
    relayer_id: &RelayerId,
    status: bool,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.update_eip1559_status(&relayer_id, status).await?;

    println!("Updated relayer {} eip1559 status to {}", relayer_id, status);

    Ok(())
}

/// Handles updating the maximum gas price limit for a relayer.
///
/// Sets a cap on the maximum gas price the relayer will pay for transactions,
/// helping to control costs and prevent excessive fees during network congestion.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer to update
/// * `cap` - Maximum gas price in wei that the relayer will pay
/// * `project_path` - Project location containing configuration
/// * `sdk` - SDK instance for API communication
///
/// # Returns
/// * `Ok(())` - Gas price limit updated successfully
/// * `Err(ConfigError)` - Update operation failed
async fn handle_update_max_gas_price(
    relayer_id: &RelayerId,
    cap: u128,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ConfigError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.update_max_gas_price(&relayer_id, cap).await?;

    println!("Updated relayer {} max gas price to {}", relayer_id, cap);

    Ok(())
}
