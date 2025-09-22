use clap::Subcommand;
use rrelayer_core::{
    common_types::{EvmAddress, PagingContext},
    relayer::types::RelayerId,
};
use rrelayer_sdk::SDK;

use crate::{commands::error::AllowlistError, console::print_table};

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

        /// Number of results to return (default: 10)
        #[clap(long, default_value = "10")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[clap(long, default_value = "0")]
        offset: u32,
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

pub async fn handle_allowlist(command: &AllowlistCommand, sdk: &SDK) -> Result<(), AllowlistError> {
    match command {
        AllowlistCommand::Add { relayer_id, address } => {
            handle_allowlist_add(relayer_id, address, sdk).await
        }
        AllowlistCommand::List { relayer_id, limit, offset } => {
            handle_allowlist_list(relayer_id, *limit, *offset, sdk).await
        }
        AllowlistCommand::Delete { relayer_id, address } => {
            handle_allowlist_delete(relayer_id, address, sdk).await
        }
    }
}

async fn handle_allowlist_add(
    relayer_id: &RelayerId,
    address: &EvmAddress,
    sdk: &SDK,
) -> Result<(), AllowlistError> {
    sdk.relayer.allowlist.add(&relayer_id, address).await?;

    println!("Allowlisted {} created for relayer {}", address.hex(), relayer_id);

    Ok(())
}

async fn handle_allowlist_list(
    relayer_id: &RelayerId,
    limit: u32,
    offset: u32,
    sdk: &SDK,
) -> Result<(), AllowlistError> {
    let paging_context = PagingContext::new(limit, offset);
    let result = sdk.relayer.allowlist.get_all(relayer_id, &paging_context).await?;

    if result.items.is_empty() {
        println!(
            "No allowlisted contracts found for relayer {} - note this means everything is allowed",
            relayer_id
        );
        return Ok(());
    }

    let mut rows = Vec::new();
    for address in result.items.iter() {
        rows.push(vec![address.to_string()]);
    }

    let headers = vec!["Allowlist Address"];

    let title = format!("{} Relayer Allowlist Addresses:", result.items.len());
    print_table(headers, rows, Some(&title), None);

    if let Some(next) = &result.next {
        println!("Use --limit {} --offset {} to see more results", next.limit, next.offset);
    }

    Ok(())
}

async fn handle_allowlist_delete(
    relayer_id: &RelayerId,
    address: &EvmAddress,
    sdk: &SDK,
) -> Result<(), AllowlistError> {
    sdk.relayer.allowlist.delete(&relayer_id, address).await?;

    println!("Allowlisted {} deleted for relayer {}", address, relayer_id);

    Ok(())
}
