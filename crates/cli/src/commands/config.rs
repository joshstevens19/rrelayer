use clap::Subcommand;
use rrelayer_core::relayer::types::RelayerId;
use rrelayer_sdk::SDK;

use crate::commands::error::ConfigError;

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

pub async fn handle_config(command: &ConfigCommand, sdk: &SDK) -> Result<(), ConfigError> {
    match command {
        ConfigCommand::Get { relayer_id } => handle_get(relayer_id, sdk).await,
        ConfigCommand::Pause { relayer_id } => handle_pause(relayer_id, sdk).await,
        ConfigCommand::Unpause { relayer_id } => handle_unpause(relayer_id, sdk).await,
        ConfigCommand::Gas { relayer_id, command } => match command {
            GasCommand::MaxPrice { cap } => {
                handle_update_max_gas_price(relayer_id, *cap, sdk).await
            }
            GasCommand::Legacy => handle_update_eip1559_status(relayer_id, false, sdk).await,
            GasCommand::Latest => handle_update_eip1559_status(relayer_id, true, sdk).await,
        },
    }
}

pub async fn handle_get(relayer_id: &RelayerId, sdk: &SDK) -> Result<(), ConfigError> {
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

async fn handle_pause(relayer_id: &RelayerId, sdk: &SDK) -> Result<(), ConfigError> {
    sdk.relayer.pause(&relayer_id).await?;

    println!("Paused relayer {}", relayer_id);

    Ok(())
}

async fn handle_unpause(relayer_id: &RelayerId, sdk: &SDK) -> Result<(), ConfigError> {
    sdk.relayer.unpause(&relayer_id).await?;

    println!("Unpaused relayer {}", relayer_id);

    Ok(())
}

async fn handle_update_eip1559_status(
    relayer_id: &RelayerId,
    status: bool,
    sdk: &SDK,
) -> Result<(), ConfigError> {
    sdk.relayer.update_eip1559_status(&relayer_id, status).await?;

    println!("Updated relayer {} eip1559 status to {}", relayer_id, status);

    Ok(())
}

async fn handle_update_max_gas_price(
    relayer_id: &RelayerId,
    cap: u128,
    sdk: &SDK,
) -> Result<(), ConfigError> {
    sdk.relayer.update_max_gas_price(&relayer_id, cap).await?;

    println!("Updated relayer {} max gas price to {}", relayer_id, cap);

    Ok(())
}
