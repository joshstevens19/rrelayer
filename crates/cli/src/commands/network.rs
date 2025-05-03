use alloy::providers::Provider;
use clap::{Args, Subcommand};
use dialoguer::{Confirm, Input};
use rrelayerr_core::{NetworkSetupConfig, create_retry_client};
use rrelayerr_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::keystore::ProjectLocation, console::print_table,
};

#[derive(Subcommand)]
pub enum NetworkCommands {
    /// Add a new network
    Add(AddArgs),
    /// List all networks
    List(ListArgs),
    /// Network-specific commands
    #[command(arg_required_else_help = true)]
    Network {
        network_name: String,
        #[command(subcommand)]
        command: NetworkSubCommands,
    },
}

#[derive(Args)]
struct AddArgs {}

#[derive(Args, Copy, Clone)]
struct ListArgs {
    #[arg(long, value_enum)]
    filter: Option<NetworkFilter>,
}

#[derive(clap::ValueEnum, Clone, Copy)]
enum NetworkFilter {
    Enabled,
    Disabled,
}

#[derive(Subcommand)]
enum NetworkSubCommands {
    /// Get gas prices for the network
    Gas,
    /// Enable the network
    Enable,
    /// Disable the network
    Disable,
}

pub async fn handle_network(
    command: &NetworkCommands,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    match &command {
        NetworkCommands::Add(_) => handle_add(project_path).await,
        NetworkCommands::List(list_args) => handle_list(list_args, project_path, sdk).await,
        NetworkCommands::Network { network_name, command } => match command {
            NetworkSubCommands::Gas => handle_gas(network_name),
            NetworkSubCommands::Enable => handle_enable(network_name),
            NetworkSubCommands::Disable => handle_disable(network_name),
        },
    }
}

async fn handle_add(project_path: &ProjectLocation) -> Result<(), Box<dyn std::error::Error>> {
    let mut setup_config = project_path.setup_config(true)?;

    let network_name: String = Input::new()
        .with_prompt("Enter network name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.trim().is_empty() { Err("Network name cannot be empty") } else { Ok(()) }
        })
        .interact_text()?;

    if setup_config.networks.iter().any(|network| network.name == network_name) {
        println!("Network '{}' already exists.", network_name);
        return Ok(());
    }

    let mut provider_urls = Vec::new();

    loop {
        let url: String = Input::new()
            .with_prompt("Enter provider URL (or press enter to finish) - you can use ${ENV_PARAM} if you wish")
            .allow_empty(true)
            .interact_text()?;

        let provider = create_retry_client(&url)
            .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;
        provider
            .get_chain_id()
            .await
            .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;

        if url.trim().is_empty() {
            if provider_urls.is_empty() {
                println!("At least one provider URL is required.");
                continue;
            }
            break;
        }

        provider_urls.push(url);

        if !Confirm::new().with_prompt("Add another provider URL?").default(true).interact()? {
            break;
        }
    }

    let block_explorer: String = Input::new()
        .with_prompt("Enter block explorer url (or press enter to use default)")
        .allow_empty(true)
        .interact_text()?;

    setup_config.networks.push(NetworkSetupConfig {
        name: network_name.clone(),
        signing_key: None,
        provider_urls,
        block_explorer_url: if block_explorer.is_empty() { None } else { Some(block_explorer) },
        gas_provider: None,
    });

    project_path.overwrite_setup_config(setup_config)?;

    println!(
        "Network '{}' added successfully - for networks to be added to rrelayerr you have to restart the server",
        network_name
    );

    Ok(())
}

async fn handle_list(
    args: &ListArgs,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let networks = if let Some(filter) = args.filter {
        match filter {
            NetworkFilter::Enabled => sdk.network.get_enabled_networks().await?,
            NetworkFilter::Disabled => sdk.network.get_disabled_networks().await?,
        }
    } else {
        sdk.network.get_all_networks().await?
    };

    if networks.is_empty() {
        println!("No networks found.");
        return Ok(());
    }

    let mut rows = Vec::new();
    for network in networks.iter() {
        let provider_str = if network.provider_urls.is_empty() {
            "None".to_string()
        } else if network.provider_urls.len() == 1 {
            network.provider_urls[0].clone()
        } else {
            format!("{} endpoints", network.provider_urls.len())
        };

        let status = if network.disabled { "Disabled" } else { "Active" };

        rows.push(vec![
            network.name.clone(),
            network.chain_id.to_string(),
            provider_str,
            status.to_string(),
        ]);
    }

    let headers = vec!["Network Name", "Chain ID", "Provider URLs", "Status"];

    let title = format!("{} Networks Available:", networks.len());
    let footer = "Tip: Run 'network info <name>' to see more details about a specific network.";

    print_table(headers, rows, Some(&title), Some(footer));

    Ok(())
}

fn handle_gas(network_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Getting gas prices for network '{}'...", network_name);
    // TODO: Implement actual gas price fetching logic
    Ok(())
}

fn handle_enable(network_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Enabling network '{}'...", network_name);
    // TODO: Implement actual network enabling logic
    Ok(())
}

fn handle_disable(network_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Disabling network '{}'...", network_name);
    // TODO: Implement actual network disabling logic
    Ok(())
}
