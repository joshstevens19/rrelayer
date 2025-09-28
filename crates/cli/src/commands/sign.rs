use clap::Subcommand;
use rrelayer_core::common_types::PagingContext;
use rrelayer_core::relayer::RelayerId;
use rrelayer_sdk::{AdminRelayerClient, Client};

use crate::commands::error::SigningError;

#[derive(Subcommand)]
pub enum SignCommand {
    /// Sign a text message
    Text {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        /// The message to sign
        #[arg(long, short = 'm')]
        message: String,
    },
    /// Sign typed data
    #[command(name = "typed-data")]
    TypedData {
        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        /// The typed data to sign as a JSON string it can also be a file location
        #[arg(long, short = 'd')]
        data: String,

        /// Read typed data from a file instead of a string
        #[arg(long, short = 'f')]
        file: bool,
    },
    /// View signing text history for a relayer
    #[command(name = "text-history")]
    TextHistory {
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
    /// View signing typed data history for a relayer
    #[command(name = "typed-data-history")]
    TypedDataHistory {
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

#[derive(Subcommand)]
pub enum TextCommand {
    /// Sign a text message
    Sign {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// The message to sign
        #[clap(required = true)]
        message: String,
    },
    /// View signing text history for a relayer
    History {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// Number of results to return (default: 100)
        #[clap(long, default_value = "100")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[clap(long, default_value = "0")]
        offset: u32,
    },
}

#[derive(Subcommand)]
pub enum TypedDataCommand {
    /// Sign typed data
    Sign {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// The typed data to sign as a JSON string it can also be a file location
        #[clap(required = true)]
        typed_data: String,

        /// Read typed data from a file instead of a string
        #[clap(long)]
        file: bool,
    },
    /// View signing typed data history for a relayer
    History {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// Number of results to return (default: 100)
        #[clap(long, default_value = "100")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[clap(long, default_value = "0")]
        offset: u32,
    },
}

pub async fn handle_sign(command: &SignCommand, client: &Client) -> Result<(), SigningError> {
    match command {
        SignCommand::Text { relayer_id, message } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_sign_text(message, &relayer_client).await
        }
        SignCommand::TypedData { relayer_id, data, file } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_sign_typed_data(data, *file, &relayer_client).await
        }
        SignCommand::TextHistory { relayer_id, limit, offset } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_text_history(*limit, *offset, &relayer_client).await
        }
        SignCommand::TypedDataHistory { relayer_id, limit, offset } => {
            let relayer_client = client.get_relayer_client(relayer_id, None).await?;
            handle_typed_data_history(*limit, *offset, &relayer_client).await
        }
    }
}

async fn handle_sign_text(message: &str, client: &AdminRelayerClient) -> Result<(), SigningError> {
    println!("Signing message with relayer {}...", client.id());
    let result = client.sign().text(message, None).await?;

    println!("\n┌─────────────────────────────────────────────────────────────────────");
    println!("│ SIGNATURE DETAILS");
    println!("├─────────────────────────────────────────────────────────────────────");
    println!("│ Relayer:         {}", client.id());
    println!("│ Message:         {}", result.message_signed);
    println!("│ Signature:       0x{}", hex::encode(result.signature.as_bytes()));
    println!("└─────────────────────────────────────────────────────────────────────");

    println!("\nThe message has been signed successfully.");

    Ok(())
}

async fn handle_sign_typed_data(
    typed_data: &str,
    file: bool,
    client: &AdminRelayerClient,
) -> Result<(), SigningError> {
    let typed_data_str =
        if file { std::fs::read_to_string(typed_data)? } else { typed_data.to_string() };

    let typed_data = match serde_json::from_str::<alloy::dyn_abi::TypedData>(&typed_data_str) {
        Ok(data) => data,
        Err(e) => {
            println!("Error parsing typed data: {}", e);
            return Err(SigningError::Json(e));
        }
    };

    let result = client.sign().typed_data(&typed_data, None).await?;

    let pretty_json =
        serde_json::to_string_pretty(&typed_data).map_err(SigningError::Json)?.to_string();

    println!("\n┌─────────────────────────────────────────────────────────────────────");
    println!("│ TYPED DATA");
    println!("├─────────────────────────────────────────────────────────────────────");

    for line in pretty_json.lines() {
        println!("│ {}", line);
    }

    println!("├─────────────────────────────────────────────────────────────────────");
    println!("│ SIGNATURE DETAILS");
    println!("├─────────────────────────────────────────────────────────────────────");
    println!("│ Relayer:         {}", client.id());

    if let Some(name) = &typed_data.domain.name {
        println!("│ Domain:          {}", name);
    }

    println!("│ Primary Type:    {}", typed_data.primary_type);

    if let Some(chain_id) = &typed_data.domain.chain_id {
        println!("│ Chain ID:        {}", chain_id);
    }

    println!("│ Signature:       0x{}", hex::encode(result.signature.as_bytes()));
    println!("└─────────────────────────────────────────────────────────────────────");

    println!("\nThe typed data has been signed successfully.");
    println!("You can verify this signature using EIP-712 verification.");

    Ok(())
}

async fn handle_text_history(
    limit: u32,
    offset: u32,
    client: &AdminRelayerClient,
) -> Result<(), SigningError> {
    println!("Retrieving text signing history for relayer {}...", client.id());

    let paging_context = PagingContext::new(limit, offset);
    let result = client.sign().text_history(&paging_context).await?;

    if result.items.is_empty() {
        println!("No text signing history found for this relayer.");
        return Ok(());
    }

    println!("\n┌─────────────────────────────────────────────────────────────────────");
    println!("│ TEXT SIGNING HISTORY ({} items)", result.items.len());
    println!("├─────────────────────────────────────────────────────────────────────");

    for (i, history) in result.items.iter().enumerate() {
        println!("│      Message: {}", history.message);
        println!("│      Signature: 0x{}", history.signature);
        println!("│      Chain ID: {}", history.chain_id);
        println!("│      Signed At: {}", history.signed_at.format("%Y-%m-%d %H:%M:%S UTC"));
        if i < result.items.len() - 1 {
            println!("│");
        }
    }

    println!("└─────────────────────────────────────────────────────────────────────");

    if let Some(next) = &result.next {
        println!("Use --limit {} --offset {} to see more results", next.limit, next.offset);
    }

    Ok(())
}

async fn handle_typed_data_history(
    limit: u32,
    offset: u32,
    client: &AdminRelayerClient,
) -> Result<(), SigningError> {
    println!("Retrieving typed data signing history for relayer {}...", client.id());

    let paging_context = PagingContext::new(limit, offset);
    let result = client.sign().typed_data_history(&paging_context).await?;

    if result.items.is_empty() {
        println!("No typed data signing history found for this relayer.");
        return Ok(());
    }

    println!("\n┌─────────────────────────────────────────────────────────────────────");
    println!("│ TYPED DATA SIGNING HISTORY ({} items)", result.items.len());
    println!("├─────────────────────────────────────────────────────────────────────");

    for (i, history) in result.items.iter().enumerate() {
        println!("│      Primary Type: {}", history.primary_type);

        println!("│      Domain Data:");
        if let Ok(pretty_domain) = serde_json::to_string_pretty(&history.domain_data) {
            for line in pretty_domain.lines() {
                println!("│        {}", line);
            }
        } else {
            println!("│        {}", history.domain_data);
        }

        println!("│      Message Data:");
        if let Ok(pretty_message) = serde_json::to_string_pretty(&history.message_data) {
            for line in pretty_message.lines() {
                println!("│        {}", line);
            }
        } else {
            println!("│        {}", history.message_data);
        }

        println!("│      Signature: 0x{}", history.signature);
        println!("│      Chain ID: {}", history.chain_id);
        println!("│      Signed At: {}", history.signed_at.format("%Y-%m-%d %H:%M:%S UTC"));

        if i < result.items.len() - 1 {
            println!("│");
        }
    }

    println!("└─────────────────────────────────────────────────────────────────────");

    if let Some(next) = &result.next {
        println!("Use --limit {} --offset {} to see more results", next.limit, next.offset);
    }

    Ok(())
}
