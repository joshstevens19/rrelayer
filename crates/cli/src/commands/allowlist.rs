use clap::Subcommand;
use rrelayer_core::{common_types::PagingContext, relayer::RelayerId};
use rrelayer_sdk::{AdminRelayerClient, Client};

use crate::{commands::error::AllowlistError, console::print_table};

#[derive(Subcommand)]
pub enum AllowlistCommand {
    /// List all allowlisted addresses
    List {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        /// Number of results to return (default: 10)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[arg(long, default_value = "0")]
        offset: u32,
    },
}

pub async fn handle_allowlist(
    command: &AllowlistCommand,
    client: &Client,
) -> Result<(), AllowlistError> {
    match command {
        AllowlistCommand::List { relayer_id, limit, offset } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_allowlist_list(relayer_id, *limit, *offset, &relayer_client).await
        }
    }
}

async fn handle_allowlist_list(
    relayer_id: &RelayerId,
    limit: u32,
    offset: u32,
    client: &AdminRelayerClient,
) -> Result<(), AllowlistError> {
    let paging_context = PagingContext::new(limit, offset);
    let result = client.allowlist().get(&paging_context).await?;

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
