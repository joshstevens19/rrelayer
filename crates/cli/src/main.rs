use std::{env, path::PathBuf, str::FromStr};

use clap::Parser;
use rrelayer_core::{load_env_from_project_path, setup_info_logger};
use rrelayer_sdk::SDK;

use crate::authentication::check_authenticate;
use crate::commands::clone;
use crate::project_location::ProjectLocation;
use crate::{
    cli_interface::{Cli, Commands},
    commands::{allowlist, auth, balance, config, create, init, list, network, sign, start, tx},
    error::CliError,
};

mod authentication;
mod cli_interface;
mod commands;
mod console;
pub use console::{print_error_message, print_success_message};
mod error;
mod project_location;

/// Resolves a path from an optional string input to an absolute canonical path.
///
/// If no override path is provided, uses the current working directory.
/// The resolved path is canonicalized to ensure it exists and is valid.
///
/// # Arguments
/// * `override_path` - Optional path string to resolve, defaults to current directory if None
///
/// # Returns
/// * `Ok(PathBuf)` - Canonicalized absolute path
/// * `Err(String)` - Error message if path resolution or canonicalization fails
fn resolve_path(override_path: &Option<String>) -> Result<PathBuf, String> {
    let path = match override_path {
        Some(path) => {
            PathBuf::from_str(path).map_err(|_| format!("Invalid path provided: '{}'", path))?
        }
        None => env::current_dir().map_err(|_| "Failed to get current directory.".to_string())?,
    };

    path.canonicalize().map_err(|e| format!("Failed to resolve path '{}': {}", path.display(), e))
}

/// Creates an SDK instance with basic auth credentials from environment variables.
///
/// # Arguments
/// * `server_url` - The server URL to connect to
///
/// # Returns
/// * `Ok(SDK)` - Configured SDK instance with basic auth
/// * `Err(String)` - Error message if environment variables are missing
fn create_sdk_with_basic_auth(server_url: String) -> Result<SDK, String> {
    let username = env::var("RRELAYER_AUTH_USERNAME")
        .map_err(|_| "Missing RRELAYER_AUTH_USERNAME environment variable".to_string())?;
    let password = env::var("RRELAYER_AUTH_PASSWORD")
        .map_err(|_| "Missing RRELAYER_AUTH_PASSWORD environment variable".to_string())?;

    Ok(SDK::new(server_url, username, password))
}

/// Main entry point for the rrelayer CLI application.
///
/// Parses command line arguments and routes to appropriate command handlers.
/// Sets up logging and handles path resolution for each command.
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(CliError)` - Command execution failed with specific error details
#[tokio::main]
async fn main() -> Result<(), CliError> {
    let cli = Cli::parse();
    setup_info_logger();

    match &cli.command {
        Commands::Init { path } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            init::handle_init(&resolved_path).await?;
        }
        Commands::Auth { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            auth::handle_auth_command(command).await;
        }
        Commands::Start { path } => {
            let resolved_path = resolve_path(&path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            start::handle_start(&resolved_path).await?;
        }
        Commands::Network { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            network::handle_network(command, &project_location, &sdk).await?;
        }
        Commands::List { path, network, limit, offset } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            list::handle_list(network, *limit, *offset, &project_location, &sdk).await?;
        }
        Commands::Config { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            config::handle_config(command, &sdk).await?;
        }
        Commands::Balance { path, relayer, token } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            balance::handle_balance(relayer, token, &sdk).await?;
        }
        Commands::Allowlist { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            allowlist::handle_allowlist(command, &sdk).await?;
        }
        Commands::Create { path, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            create::handle_create(name, network, &project_location, &sdk).await?;
        }
        Commands::Clone { path, relayer, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            clone::handle_clone(relayer, name, network, &project_location, &sdk).await?;
        }
        Commands::Sign { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            sign::handle_sign(command, &sdk).await?;
        }
        Commands::Tx { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            check_authenticate(&sdk).await?;

            tx::handle_tx(command, &sdk).await?;
        }
    }

    Ok(())
}
