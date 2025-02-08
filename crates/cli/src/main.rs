use clap::Parser;

use crate::{
    cli_interface::{Cli, Commands},
    commands::{
        allowlist, api_key, balance, config, create, init, list, network, sign, start, stop, tx,
        user,
    },
};

mod authentication;
mod cli_interface;
mod commands;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    authentication::check_token_and_refresh_if_needed().await?;

    match &cli.command {
        Commands::Init => {
            if let Err(e) = init::handle_init() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Start(args) => {
            start::handle_start(args).await?;
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
            if let Err(e) = config::handle_config(relayer_id, command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Balance(args) => {
            if let Err(e) = balance::handle_balance(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::ApiKey { relayer_id, command } => {
            if let Err(e) = api_key::handle_api_key(relayer_id, command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Allowlist { relayer_id, command } => {
            if let Err(e) = allowlist::handle_allowlist(relayer_id, command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Create(args) => {
            if let Err(e) = create::handle_create(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Sign { relayer_id, command } => {
            if let Err(e) = sign::handle_sign(relayer_id, command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Tx { command } => {
            if let Err(e) = tx::handle_tx(command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::User { command } => {
            if let Err(e) = user::handle_user(command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
