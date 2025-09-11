use clap::Subcommand;
use rrelayer_core::relayer::types::RelayerId;
use rrelayer_sdk::SDK;

use crate::commands::error::SigningError;

#[derive(Subcommand)]
pub enum SignCommand {
    /// Sign a text message
    Text {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// The message to sign
        #[clap(required = true)]
        message: String,
    },
    /// Sign typed data
    TypedData {
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
}

pub async fn handle_sign(command: &SignCommand, sdk: &SDK) -> Result<(), SigningError> {
    match command {
        SignCommand::Text { relayer_id, message } => {
            handle_sign_text(relayer_id, message, sdk).await
        }
        SignCommand::TypedData { relayer_id, typed_data, file } => {
            handle_sign_typed_data(relayer_id, typed_data, *file, sdk).await
        }
    }
}
async fn handle_sign_text(
    relayer_id: &RelayerId,
    message: &str,
    sdk: &SDK,
) -> Result<(), SigningError> {
    println!("Signing message with relayer {}...", relayer_id);
    let result = sdk.sign.sign_text(relayer_id, message).await?;

    println!("\n┌─────────────────────────────────────────────────────────────────────");
    println!("│ SIGNATURE DETAILS");
    println!("├─────────────────────────────────────────────────────────────────────");
    println!("│ Relayer:         {}", relayer_id);
    println!("│ Message:         {}", result.message_signed);
    println!("│ Signature:       0x{}", hex::encode(result.signature.as_bytes()));
    println!("└─────────────────────────────────────────────────────────────────────");

    println!("\nThe message has been signed successfully.");

    Ok(())
}

async fn handle_sign_typed_data(
    relayer_id: &RelayerId,
    typed_data: &str,
    file: bool,
    sdk: &SDK,
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

    let result = sdk.sign.sign_typed_data(relayer_id, &typed_data).await?;

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
    println!("│ Relayer:         {}", relayer_id);

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
