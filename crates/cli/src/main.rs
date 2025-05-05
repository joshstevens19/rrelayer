use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use rrelayerr_core::{load_env_from_project_path, setup_info_logger};
use rrelayerr_sdk::SDK;

use crate::{
    cli_interface::{Cli, Commands},
    commands::{
        allowlist, api_key, auth, balance, config, create, init, keystore,
        keystore::ProjectLocation, list, network, sign, start, tx, user,
    },
    console::print_error_message,
};

mod authentication;
mod cli_interface;
mod commands;
mod console;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            let mut sdk = SDK::new(project_location.get_api_url()?);

            network::handle_network(command, &project_location, &mut sdk).await?;
        }
        Commands::List { path, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            list::handle_list(network, &project_location, &mut sdk).await?;
        }
        Commands::Config { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            config::handle_config(command, &project_location, &mut sdk).await?;
        }
        Commands::Balance { path, relayer, token } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            balance::handle_balance(relayer, token, &project_location, &mut sdk).await?;
        }
        Commands::ApiKey { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            api_key::handle_api_key(command, &project_location, &mut sdk).await?;
        }
        Commands::Allowlist { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            allowlist::handle_allowlist(command, &project_location, &mut sdk).await?;
        }
        Commands::Create { path, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            create::handle_create(name, network, &project_location, &mut sdk).await?;
        }
        Commands::Sign { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            sign::handle_sign(command, &project_location, &mut sdk).await?;
        }
        Commands::Tx { command } => {
            tx::handle_tx(command).await?;
        }
        Commands::User { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let mut sdk = SDK::new(project_location.get_api_url()?);

            user::handle_user(command, &project_location, &mut sdk).await?;
        }
        Commands::Keystore { command } => {
            keystore::handle_keystore_command(command).await?;
        }
    }

    Ok(())
}
