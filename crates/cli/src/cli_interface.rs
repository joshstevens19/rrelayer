use clap::{Parser, Subcommand};

use crate::commands::{
    allowlist, api_key, auth::AuthCommand, balance, config, create, keystore::KeystoreCommand,
    list, network::NetworkCommands, sign, tx, user,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new relayer project
    Init {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,
    },
    Auth {
        #[clap(long, short)]
        path: Option<String>,

        #[clap(subcommand)]
        command: AuthCommand,
    },
    /// Keystore management commands
    Keystore {
        #[clap(subcommand)]
        command: KeystoreCommand,
    },
    /// Start the relayer service
    Start {
        #[clap(long, short)]
        path: Option<String>,
    },
    /// Manage network configurations and settings
    Network {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: NetworkCommands,
    },
    /// List all configured relayers
    List(list::ListArgs),
    /// Configure operations for a specific relayer
    Config {
        /// The unique identifier of the relayer
        relayer_id: String,
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    /// Check the balance of a relayer's account
    Balance(balance::BalanceArgs),
    /// Manage API keys for relayer access
    ApiKey {
        /// The unique identifier of the relayer
        relayer_id: String,
        #[command(subcommand)]
        command: api_key::ApiKeyCommand,
    },
    /// Manage allowlist addresses for restricted access
    Allowlist {
        /// The unique identifier of the relayer
        relayer_id: String,
        #[command(subcommand)]
        command: allowlist::AllowlistCommand,
    },
    /// Create a new relayer client instance
    Create(create::CreateArgs),
    /// Sign messages and transactions
    Sign {
        /// The unique identifier of the relayer
        relayer_id: String,
        #[command(subcommand)]
        command: sign::SignCommand,
    },
    /// Manage and monitor transactions
    Tx {
        #[command(subcommand)]
        command: tx::TxCommand,
    },
    /// Manage user access and permissions
    User {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: user::UserCommand,
    },
}
