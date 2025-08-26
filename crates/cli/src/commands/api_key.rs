use clap::Subcommand;
use rrelayer_core::{
    common_types::{ApiKey, PagingContext},
    relayer::types::RelayerId,
};
use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::error::ApiKeyError,
    commands::keystore::ProjectLocation, console::print_table,
};

#[derive(Subcommand)]
pub enum ApiKeyCommand {
    /// Add a new API key
    Add {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// List all API keys
    List {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// Delete an API key
    Delete {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// API key
        #[clap(required = true)]
        api_key: ApiKey,
    },
}

/// Handles API key command routing and execution.
///
/// Routes the API key command to the appropriate handler function
/// based on the command type (Add, List, or Delete).
///
/// # Arguments
/// * `command` - The API key command to execute
/// * `project_path` - Project location containing configuration
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(ApiKeyError)` - Command execution failed
pub async fn handle_api_key(
    command: &ApiKeyCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ApiKeyError> {
    match command {
        ApiKeyCommand::Add { relayer_id } => {
            handle_api_key_add(relayer_id, project_path, sdk).await
        }
        ApiKeyCommand::List { relayer_id } => {
            handle_api_key_list(relayer_id, project_path, sdk).await
        }
        ApiKeyCommand::Delete { relayer_id, api_key } => {
            handle_api_key_delete(relayer_id, api_key, project_path, sdk).await
        }
    }
}

/// Creates a new API key for the specified relayer.
///
/// Authenticates the user and generates a new API key that can be used
/// to authenticate API requests for the relayer.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer
/// * `project_path` - Project location for authentication
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - API key created successfully
/// * `Err(ApiKeyError)` - Operation failed due to authentication or API error
async fn handle_api_key_add(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ApiKeyError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let result = sdk.relayer.api_keys.create(&relayer_id).await?;

    println!("API key: {} created for relayer {}", result.api_key, relayer_id);

    Ok(())
}

/// Lists all API keys for the specified relayer.
///
/// Authenticates the user and retrieves all API keys associated with
/// the relayer, displaying them in a formatted table.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer
/// * `project_path` - Project location for authentication
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - API keys displayed successfully
/// * `Err(ApiKeyError)` - Operation failed due to authentication or API error
async fn handle_api_key_list(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ApiKeyError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let api_keys = sdk
        .relayer
        .api_keys
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

    if api_keys.is_empty() {
        println!("No API keys found for relayer {}", relayer_id);
        return Ok(());
    }

    let mut rows = Vec::new();
    for api_key in api_keys.iter() {
        rows.push(vec![api_key.to_string()]);
    }

    let headers = vec!["API Key"];

    let title = format!("{} Relayer API Keys:", api_keys.len());
    print_table(headers, rows, Some(&title), None);

    Ok(())
}

/// Deletes an API key for the specified relayer.
///
/// Authenticates the user and revokes the specified API key, making it
/// no longer valid for API authentication.
///
/// # Arguments
/// * `relayer_id` - Unique identifier of the relayer
/// * `api_key` - The API key to delete
/// * `project_path` - Project location for authentication
/// * `sdk` - Mutable reference to the SDK for API operations
///
/// # Returns
/// * `Ok(())` - API key deleted successfully
/// * `Err(ApiKeyError)` - Operation failed due to authentication or API error
async fn handle_api_key_delete(
    relayer_id: &RelayerId,
    api_key: &ApiKey,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), ApiKeyError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.api_keys.delete(&relayer_id, api_key).await?;

    println!("API key: {} deleted for relayer {}", api_key, relayer_id);

    Ok(())
}
