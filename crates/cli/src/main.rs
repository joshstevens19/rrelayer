use std::{env, path::PathBuf, str::FromStr};

#[cfg(feature = "jemalloc")]
use tikv_jemallocator::Jemalloc;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

use clap::Parser;
use rrelayer_core::{load_env_from_project_path, setup_info_logger};

use crate::authentication::check_authenticate;
use crate::commands::clone;
use crate::project_location::ProjectLocation;
use crate::{
    cli_interface::{Cli, Commands},
    commands::{allowlist, auth, balance, config, create, list, network, new, sign, start, tx},
    error::CliError,
};

mod authentication;
mod cli_interface;
mod commands;
mod console;
mod credentials;
use crate::commands::error::ProjectLocationError;
pub use console::{print_error_message, print_success_message};
use rrelayer::{Client, CreateClientAuth, CreateClientConfig};

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
) -> Result<Client, ProjectLocationError> {
    // use std::env;

    // // Try environment variables first (for backward compatibility)
    // if let (Ok(username), Ok(password)) =
    //     (env::var("RRELAYER_AUTH_USERNAME"), env::var("RRELAYER_AUTH_PASSWORD"))
    // {
    //     return Ok(Client::new(CreateClientConfig {
    //         server_url: project_location.get_api_url()?,
    //         auth: CreateClientAuth { username, password },
    //     }));
    // }

    // Try to read from rrelayer.yaml file
    if let Ok(setup_config) = project_location.setup_config(false) {
        return Ok(Client::new(CreateClientConfig {
            server_url: project_location.get_api_url()?,
            auth: CreateClientAuth {
                username: setup_config.api_config.authentication_username,
                password: setup_config.api_config.authentication_password,
            },
        }));
    }

    Err(ProjectLocationError::ProjectConfig("You can only run rrelayer in the root of the rrelayer project (where the rrelayer.yaml is)".to_string()))

    // // Try stored credentials as fallback
    // let default_profile = "default";
    // match credentials::load_credentials(default_profile) {
    //     Ok(creds) => Ok(Client::new(CreateClientConfig {
    //         server_url: creds.api_url,
    //         auth: CreateClientAuth { username: creds.username, password: creds.password },
    //     })),
    //     Err(_) => {
    //         return Err(ProjectLocationError::ProjectConfig(
    //             "No authentication credentials found. Please either:\n\
    //             1. Set RRELAYER_AUTH_USERNAME and RRELAYER_AUTH_PASSWORD environment variables\n\
    //             2. Run rrelayer where rrelayer.yaml exists with authentication config\n\
    //             3. Run 'rrelayer auth login' to store credentials securely"
    //                 .to_string(),
    //         ));
    //     }
    // }
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let cli = Cli::parse();
    setup_info_logger();

    match &cli.command {
        Commands::New { path } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            new::handle_init(&resolved_path).await?;
        }
        Commands::Auth { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            auth::handle_auth_command(command).await?;
        }
        Commands::Start { path } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            start::handle_start(&resolved_path).await?;
        }
        Commands::Network { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            network::handle_network(command, &project_location, &client).await?;
        }
        Commands::List { path, network, limit, offset } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            list::handle_list(network, *limit, *offset, &project_location, &client).await?;
        }
        Commands::Config { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            config::handle_config(command, &client).await?;
        }
        Commands::Balance { path, relayer_id, token } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            balance::handle_balance(relayer_id, token, &client).await?;
        }
        Commands::Allowlist { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            allowlist::handle_allowlist(command, &client).await?;
        }
        Commands::Create { path, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            create::handle_create(name, network, &project_location, &client).await?;
        }
        Commands::Clone { path, relayer_id, name, network } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            clone::handle_clone(relayer_id, name, network, &project_location, &client).await?;
        }
        Commands::Sign { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            sign::handle_sign(command, &client).await?;
        }
        Commands::Tx { path, command } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            let project_location = ProjectLocation::new(resolved_path);
            let client = create_sdk_with_basic_auth(&project_location)?;

            check_authenticate(&client).await?;

            tx::handle_tx(command, &client).await?;
        }
    }

    Ok(())
}
