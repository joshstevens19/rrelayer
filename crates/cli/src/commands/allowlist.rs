use clap::Subcommand;
use rrelayerr_core::{
    common_types::{EvmAddress, PagingContext},
    relayer::types::RelayerId,
};
use rrelayerr_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::keystore::ProjectLocation, console::print_table,
};

#[derive(Subcommand)]
pub enum AllowlistCommand {
    /// Add an address to allowlist
    Add {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// The address to allow this relayer to send transactions to
        #[clap(required = true)]
        address: EvmAddress,
    },
    /// List all allowlisted addresses
    List {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// Delete an address from the allowlist
    Delete {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// The address to remove from the allowlist
        #[clap(required = true)]
        address: EvmAddress,
    },
}

pub async fn handle_allowlist(
    command: &AllowlistCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        AllowlistCommand::Add { relayer_id, address } => {
            handle_allowlist_add(relayer_id, address, project_path, sdk).await
        }
        AllowlistCommand::List { relayer_id } => {
            handle_allowlist_list(relayer_id, project_path, sdk).await
        }
        AllowlistCommand::Delete { relayer_id, address } => {
            handle_allowlist_delete(relayer_id, address, project_path, sdk).await
        }
    }
}

async fn handle_allowlist_add(
    relayer_id: &RelayerId,
    address: &EvmAddress,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.allowlist.add(&relayer_id, address).await?;

    println!("Allowlisted {} created for relayer {}", address.hex(), relayer_id);

    Ok(())
}

async fn handle_allowlist_list(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let addresses = sdk
        .relayer
        .allowlist
        .get_all(
            relayer_id,
            &PagingContext {
                // TODO: handle paging later
                limit: 1000,
                offset: 0,
            },
        )
        .await?
        .items;

    if addresses.is_empty() {
        println!(
            "No allowlisted contracts found for relayer {} - note this means everything is allowed",
            relayer_id
        );
        return Ok(());
    }

    let mut rows = Vec::new();
    for address in addresses.iter() {
        rows.push(vec![address.to_string()]);
    }

    let headers = vec!["Allowlist Address"];

    let title = format!("{} Relayer Allowlist Addresses:", addresses.len());
    print_table(headers, rows, Some(&title), None);

    Ok(())
}

async fn handle_allowlist_delete(
    relayer_id: &RelayerId,
    address: &EvmAddress,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.allowlist.delete(&relayer_id, address).await?;

    println!("Allowlisted {} deleted for relayer {}", address, relayer_id);

    Ok(())
}
