use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use rrelayerr::{load_env_from_project_path, setup_info_logger};

use crate::{
    cli_interface::{Cli, Commands},
    commands::{
        allowlist, api_key, balance, config, create, init, list, network, sign, start, stop, tx,
        user,
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
            std::env::current_dir()
                .map_err(|_| "Failed to get current directory.".to_string())?
        }
    };

    path.canonicalize()
        .map_err(|e| format!("Failed to resolve path '{}': {}", path.display(), e))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    setup_info_logger();

    authentication::check_token_and_refresh_if_needed().await?;

    match &cli.command {
        Commands::Init { path } => {
            let resolved_path = resolve_path(path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            init::handle_init(&resolved_path).await?;
        }
        Commands::Start(args) => {
            let resolved_path = resolve_path(&args.path).inspect_err(|e| print_error_message(e))?;
            load_env_from_project_path(&resolved_path);

            start::handle_start(args, &resolved_path).await?;
        }
        Commands::Stop => {
            stop::handle_stop().await?;
        }
        Commands::Network(args) => {
            network::handle_network(args).await?;
        }
        Commands::List(args) => {
            list::handle_list(args).await?;
        }
        Commands::Config { relayer_id, command } => {
            config::handle_config(relayer_id, command).await?;
        }
        Commands::Balance(args) => {
            balance::handle_balance(args).await?;
        }
        Commands::ApiKey { relayer_id, command } => {
            api_key::handle_api_key(relayer_id, command).await?;
        }
        Commands::Allowlist { relayer_id, command } => {
            allowlist::handle_allowlist(relayer_id, command).await?;
        }
        Commands::Create(args) => {
            create::handle_create(args).await?;
        }
        Commands::Sign { relayer_id, command } => {
            sign::handle_sign(relayer_id, command).await?;
        }
        Commands::Tx { command } => {
            tx::handle_tx(command).await?;
        }
        Commands::User { command } => {
            user::handle_user(command).await?;
        }
    }

    Ok(())
}
