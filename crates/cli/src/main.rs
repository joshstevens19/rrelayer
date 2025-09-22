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
use crate::commands::error::ProjectLocationError;
pub use console::{print_error_message, print_success_message};

mod error;
mod project_location;

fn resolve_path(override_path: &Option<String>) -> Result<PathBuf, String> {
    let path = match override_path {
        Some(path) => {
            PathBuf::from_str(path).map_err(|_| format!("Invalid path provided: '{}'", path))?
        }
        None => env::current_dir().map_err(|_| "Failed to get current directory.".to_string())?,
    };

    path.canonicalize().map_err(|e| format!("Failed to resolve path '{}': {}", path.display(), e))
}

fn create_sdk_with_basic_auth(
    project_location: &ProjectLocation,
) -> Result<SDK, ProjectLocationError> {
    let setup_config = project_location.setup_config(false)?;

    Ok(SDK::new(
        project_location.get_api_url()?,
        setup_config.api_config.authentication_username,
        setup_config.api_config.authentication_password,
    ))
}

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
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            network::handle_network(command, &project_location, &sdk).await?;
        }
        Commands::List { path, network, limit, offset } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            list::handle_list(network, *limit, *offset, &project_location, &sdk).await?;
        }
        Commands::Config { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            config::handle_config(command, &sdk).await?;
        }
        Commands::Balance { path, relayer, token } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            balance::handle_balance(relayer, token, &sdk).await?;
        }
        Commands::Allowlist { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            allowlist::handle_allowlist(command, &sdk).await?;
        }
        Commands::Create { path, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            create::handle_create(name, network, &project_location, &sdk).await?;
        }
        Commands::Clone { path, relayer, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            clone::handle_clone(relayer, name, network, &project_location, &sdk).await?;
        }
        Commands::Sign { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            sign::handle_sign(command, &sdk).await?;
        }
        Commands::Tx { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let sdk = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&sdk).await?;

            tx::handle_tx(command, &sdk).await?;
        }
    }

    Ok(())
}
