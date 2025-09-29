use crate::commands::error::ConfigError;
use clap::Subcommand;
use rrelayer::{AdminRelayerClient, Client};
use rrelayer_core::relayer::RelayerId;

#[derive(Subcommand)]
pub enum GasCommand {
    /// Set maximum gas price cap
    #[command(name = "max-price")]
    MaxPrice {
        /// Maximum gas price in wei
        #[arg(long, short = 'p')]
        price: u128,
    },
    /// Enable legacy transactions gas support (non-EIP-1559)
    Legacy,
    /// Enable EIP-1559 gas standard for transactions
    Latest,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Get detailed information about the relayer
    Get {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,
    },
    /// Pause operations for a specific relayer
    Pause {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,
    },
    /// Resume operations for a paused relayer
    Unpause {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,
    },
    /// Manage gas settings for a relayer
    Gas {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        #[command(subcommand)]
        command: GasCommand,
    },
}

pub async fn handle_config(command: &ConfigCommand, client: &Client) -> Result<(), ConfigError> {
    match command {
        ConfigCommand::Get { relayer_id } => handle_get(relayer_id, client).await,
        ConfigCommand::Pause { relayer_id } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_pause(&relayer_client).await
        }
        ConfigCommand::Unpause { relayer_id } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_unpause(&relayer_client).await
        }
        ConfigCommand::Gas { relayer_id, command } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            match command {
                GasCommand::MaxPrice { price } => {
                    handle_update_max_gas_price(*price, &relayer_client).await
                }
                GasCommand::Legacy => handle_update_eip1559_status(false, &relayer_client).await,
                GasCommand::Latest => handle_update_eip1559_status(true, &relayer_client).await,
            }
        }
    }
}

pub async fn handle_get(relayer_id: &RelayerId, client: &Client) -> Result<(), ConfigError> {
    let result = client.relayer().get(relayer_id).await?;
    match result {
        Some(result) => {
            let relayer = &result.relayer;
            let provider_urls = &result.provider_urls;

            let created_at = relayer.created_at.format("%Y-%m-%d %H:%M:%S UTC");

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

async fn handle_pause(client: &AdminRelayerClient) -> Result<(), ConfigError> {
    client.pause().await?;

    println!("Paused relayer {}", client.id());

    Ok(())
}

async fn handle_unpause(client: &AdminRelayerClient) -> Result<(), ConfigError> {
    client.unpause().await?;

    println!("Unpaused relayer {}", client.id());

    Ok(())
}

async fn handle_update_eip1559_status(
    status: bool,
    client: &AdminRelayerClient,
) -> Result<(), ConfigError> {
    client.update_eip1559_status(status).await?;

    println!("Updated relayer {} eip1559 status to {}", client.id(), status);

    Ok(())
}

async fn handle_update_max_gas_price(
    cap: u128,
    client: &AdminRelayerClient,
) -> Result<(), ConfigError> {
    client.update_max_gas_price(cap).await?;

    println!("Updated relayer {} max gas price to {}", client.id(), cap);

    Ok(())
}
