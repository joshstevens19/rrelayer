use std::io::{self};

use clap::{Args, Subcommand};
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
    /// Network specific commands
    #[command(arg_required_else_help = true)]
    Network {
        network_name: String,
        #[command(subcommand)]
        command: NetworkSubCommands,
    },
}

#[derive(Args)]
struct AddArgs {}

#[derive(Args)]
struct ListArgs {
    #[arg(long, value_enum)]
    filter: Option<NetworkFilter>,
}

#[derive(clap::ValueEnum, Clone)]
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
        NetworkCommands::Add(_) => handle_add(),
        NetworkCommands::List(list_args) => handle_list(list_args, project_path, sdk).await,
        NetworkCommands::Network { network_name, command } => match command {
            NetworkSubCommands::Gas => handle_gas(network_name),
            NetworkSubCommands::Enable => handle_enable(network_name),
            NetworkSubCommands::Disable => handle_disable(network_name),
        },
    }
}

fn handle_add() -> Result<(), Box<dyn std::error::Error>> {
    println!("Enter network name:");
    let mut network_name = String::new();
    io::stdin().read_line(&mut network_name)?;
    let network_name = network_name.trim();

    let mut provider_urls = Vec::new();
    loop {
        println!("Enter provider URL (or press enter to finish):");
        let mut url = String::new();
        io::stdin().read_line(&mut url)?;
        let url = url.trim();

        if url.is_empty() {
            if provider_urls.is_empty() {
                println!("At least one provider URL is required.");
                continue;
            }
            break;
        }
        provider_urls.push(url.to_string());
    }

    println!("Select gas provider:");
    println!("1. Infura");
    println!("2. Tenderly");
    println!("3. Built in");
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let gas_provider = match choice.trim() {
        "1" => "infura",
        "2" => "tenderly",
        "3" => "built_in",
        _ => return Err("Invalid gas provider choice".into()),
    };

    // Save network configuration
    // TODO: Implement actual config saving logic
    println!("Network '{}' added successfully!", network_name);
    Ok(())
}

async fn handle_list(
    args: &ListArgs,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let networks = sdk.network.get_all_networks().await?;

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
