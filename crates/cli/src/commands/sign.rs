use clap::Subcommand;
use rrelayer_core::relayer::types::RelayerId;
use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::error::SigningError,
    commands::keystore::ProjectLocation,
};

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

/// Handles signing commands by dispatching to the appropriate handler.
///
/// Routes sign commands to either text message signing or typed data signing
/// based on the command variant.
///
/// # Arguments
/// * `command` - The signing command to execute
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Signing completed successfully
/// * `Err(SigningError)` - Authentication failed or signing failed
pub async fn handle_sign(
    command: &SignCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), SigningError> {
    match command {
        SignCommand::Text { relayer_id, message } => {
            handle_sign_text(relayer_id, message, project_path, sdk).await
        }
        SignCommand::TypedData { relayer_id, typed_data, file } => {
            handle_sign_typed_data(relayer_id, typed_data, *file, project_path, sdk).await
        }
    }
}

/// Signs a plain text message using the specified relayer.
///
/// Authenticates the user, signs the provided message with the relayer's private key,
/// and displays the signature details in a formatted box.
///
/// # Arguments
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `message` - The text message to sign
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Message signed successfully
/// * `Err(SigningError)` - Authentication failed or signing failed
async fn handle_sign_text(
    relayer_id: &RelayerId,
    message: &str,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), SigningError> {
    handle_authenticate(sdk, "account1", project_path).await?;

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

/// Signs EIP-712 typed data using the specified relayer.
///
/// Authenticates the user, parses the typed data (either from a string or file),
/// signs it with the relayer's private key, and displays both the typed data and
/// signature details in a formatted output.
///
/// # Arguments
/// * `relayer_id` - The unique identifier of the relayer to use for signing
/// * `typed_data` - The typed data as a JSON string or file path
/// * `file` - Whether to read typed data from a file instead of parsing as string
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Typed data signed successfully
/// * `Err(SigningError)` - Authentication failed, invalid JSON, or signing failed
async fn handle_sign_typed_data(
    relayer_id: &RelayerId,
    typed_data: &str,
    file: bool,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), SigningError> {
    handle_authenticate(sdk, "account1", project_path).await?;

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
