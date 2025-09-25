use alloy::providers::Provider;
use clap::{Args, Subcommand};
use dialoguer::{Confirm, Input};
use prettytable::{Cell, Row, Table, format};
use rrelayer_core::{NetworkSetupConfig, create_retry_client, gas::GasPriceResult, get_chain_id};
use rrelayer_sdk::SDK;

use crate::project_location::ProjectLocation;
use crate::{commands::error::NetworkError, console::print_table};

#[derive(Subcommand)]
pub enum NetworkCommands {
    /// Add a new network
    Add(AddArgs),
    /// List all networks
    List,
    Gas {
        /// The network name
        #[arg(long, short = 'n')]
        name: String,
    },
}

#[derive(Args)]
pub struct AddArgs {}

pub async fn handle_network(
    command: &NetworkCommands,
    project_path: &ProjectLocation,
    sdk: &SDK,
) -> Result<(), NetworkError> {
    match &command {
        NetworkCommands::Add(_) => handle_add(project_path).await,
        NetworkCommands::List => handle_list(sdk).await,
        NetworkCommands::Gas { name: network_name } => {
            handle_gas(network_name, project_path, sdk).await
        }
    }
}

async fn handle_add(project_path: &ProjectLocation) -> Result<(), NetworkError> {
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

        let provider = create_retry_client(&url).await.map_err(|e| {
            NetworkError::InvalidConfig(format!(
                "RPC provider is not valid as cannot get chain ID: {}",
                e
            ))
        })?;
        provider.get_chain_id().await.map_err(|e| {
            NetworkError::ConnectionFailed(format!(
                "RPC provider is not valid as cannot get chain ID: {}",
                e
            ))
        })?;

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
        chain_id: get_chain_id(provider_urls.first().unwrap())
            .await
            .expect("Could not read from rpc for the chain id"),
        signing_provider: None,
        provider_urls,
        block_explorer_url: if block_explorer.is_empty() { None } else { Some(block_explorer) },
        gas_provider: None,
        automatic_top_up: None,
        confirmations: None,
        permissions: None,
        api_keys: None,
    });

    project_path.overwrite_setup_config(setup_config)?;

    println!(
        "Network '{}' added successfully - for networks to be added to rrelayer you have to restart the server",
        network_name
    );

    Ok(())
}

async fn handle_list(sdk: &SDK) -> Result<(), NetworkError> {
    let networks = sdk.network.get_all_networks().await?;

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

        rows.push(vec![network.name.clone(), network.chain_id.to_string(), provider_str]);
    }

    let headers = vec!["Network Name", "Chain ID", "Provider URLs"];

    let title = format!("{} Networks Available:", networks.len());
    let footer = "Tip: Run 'network info <name>' to see more details about a specific network.";

    print_table(headers, rows, Some(&title), Some(footer));

    Ok(())
}

fn get_wait_time(result: &GasPriceResult) -> String {
    match (result.min_wait_time_estimate, result.max_wait_time_estimate) {
        (Some(min), Some(max)) => format!("{}-{} sec", min, max),
        (Some(min), None) => format!("Min: {} sec", min),
        (None, Some(max)) => format!("Max: {} sec", max),
        (None, None) => "Unknown".to_string(),
    }
}

async fn handle_gas(
    network_name: &str,
    project_path: &ProjectLocation,
    sdk: &SDK,
) -> Result<(), NetworkError> {
    let chain_id = get_chain_id_for_network(network_name, project_path).await?;

    let gas_prices = sdk.gas.get_gas_prices(chain_id).await?;
    match gas_prices {
        None => {
            println!("No gas prices found for chain ID: {}", chain_id);
            return Ok(());
        }
        Some(gas_prices) => {
            let mut table = Table::new();
            table.set_format(*format::consts::FORMAT_BOX_CHARS);

            table.add_row(Row::new(vec![
                Cell::new(&format!("Gas Prices for {} (Chain ID: {})", network_name, chain_id))
                    .style_spec("b")
                    .with_hspan(4),
            ]));

            table.add_row(Row::new(vec![
                Cell::new("Priority").style_spec("b"),
                Cell::new("Max Priority Fee").style_spec("b"),
                Cell::new("Max Fee").style_spec("b"),
                Cell::new("Wait Time").style_spec("b"),
            ]));

            table.add_row(Row::new(vec![
                Cell::new("Slow"),
                Cell::new(&gas_prices.slow.max_priority_fee.into_u128().to_string()),
                Cell::new(&gas_prices.slow.max_fee.into_u128().to_string()),
                Cell::new(&get_wait_time(&gas_prices.slow)),
            ]));

            table.add_row(Row::new(vec![
                Cell::new("Medium"),
                Cell::new(&gas_prices.medium.max_priority_fee.into_u128().to_string()),
                Cell::new(&gas_prices.medium.max_fee.into_u128().to_string()),
                Cell::new(&get_wait_time(&gas_prices.medium)),
            ]));

            table.add_row(Row::new(vec![
                Cell::new("Fast"),
                Cell::new(&gas_prices.fast.max_priority_fee.into_u128().to_string()),
                Cell::new(&gas_prices.fast.max_fee.into_u128().to_string()),
                Cell::new(&get_wait_time(&gas_prices.fast)),
            ]));

            table.add_row(Row::new(vec![
                Cell::new("Super Fast"),
                Cell::new(&gas_prices.super_fast.max_priority_fee.into_u128().to_string()),
                Cell::new(&gas_prices.super_fast.max_fee.into_u128().to_string()),
                Cell::new(&get_wait_time(&gas_prices.super_fast)),
            ]));

            table.printstd();
        }
    }

    Ok(())
}

pub async fn get_chain_id_for_network(
    network_name: &str,
    project_path: &ProjectLocation,
) -> Result<u64, Box<dyn std::error::Error>> {
    let setup_config = project_path.setup_config(false)?;
    let provider_url = setup_config
        .networks
        .iter()
        .find(|network| network.name == network_name)
        .ok_or_else(|| format!("Network not found: {}", network_name))?
        .provider_urls[0]
        .clone();

    let provider = create_retry_client(&provider_url)
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;
    let chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;

    Ok(chain_id)
}
