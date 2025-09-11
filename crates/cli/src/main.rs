use std::{env, path::PathBuf, str::FromStr};

use clap::Parser;
use rrelayer_core::{load_env_from_project_path, setup_info_logger};
use rrelayer_sdk::SDK;

use crate::commands::clone;
use crate::{
    cli_interface::{Cli, Commands},
    commands::{
        allowlist, auth, balance, config, create, init, keystore,
        keystore::ProjectLocation, list, network, sign, start, tx, user,
    },
    console::print_error_message,
    error::CliError,
};

mod authentication;
mod cli_interface;
mod commands;
mod console;
mod error;

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
        None => {
            std::env::current_dir().map_err(|_| "Failed to get current directory.".to_string())?
        }
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
            auth::handle_auth_command(command, resolved_path).await?;
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
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            network::handle_network(command, &project_location, &mut sdk).await?;
        }
        Commands::List { path, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            list::handle_list(network, &project_location, &mut sdk).await?;
        }
        Commands::Config { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            config::handle_config(command, &project_location, &mut sdk).await?;
        }
        Commands::Balance { path, relayer, token } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            balance::handle_balance(relayer, token, &project_location, &mut sdk).await?;
        }
        Commands::Allowlist { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            allowlist::handle_allowlist(command, &project_location, &mut sdk).await?;
        }
        Commands::Create { path, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            create::handle_create(name, network, &project_location, &mut sdk).await?;
        }
        Commands::Clone { path, relayer, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            clone::handle_clone(relayer, name, network, &project_location, &mut sdk).await?;
        }
        Commands::Sign { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            sign::handle_sign(command, &project_location, &mut sdk).await?;
        }
        Commands::Tx { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            tx::handle_tx(command, &project_location, &mut sdk).await?;
        }
        Commands::User { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = create_sdk_with_basic_auth(project_location.get_api_url()?)
                .map_err(|e| CliError::Authentication(e))?;

            user::handle_user(command, &project_location, &mut sdk).await?;
        }
        Commands::Keystore { command } => {
            keystore::handle_keystore_command(command).await?;
        }
    }

    Ok(())
}
