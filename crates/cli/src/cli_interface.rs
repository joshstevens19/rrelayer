use clap::{Parser, Subcommand};
use rrelayer_core::{common_types::EvmAddress, relayer::types::RelayerId};

use crate::commands::{
    allowlist, api_key, auth::AuthCommand, config, keystore::KeystoreCommand,
    network::NetworkCommands, sign, tx, user,
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
    /// Clone an existing relayer private key to another network
    Clone {
        #[clap(long, short)]
        path: Option<String>,

        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer: RelayerId,

        /// The new relayer name
        #[clap(required = true)]
        name: String,

        /// Network to assign it to
        #[arg(required = true)]
        network: String,
    },
    /// Authenticate with rrelayer
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
    List {
        #[clap(long, short)]
        path: Option<String>,

        #[arg(long, short)]
        network: Option<String>,
    },
    /// Configure operations for a specific relayer
    Config {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    /// Check the balance of a relayer's account
    Balance {
        #[clap(long, short)]
        path: Option<String>,

        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer: RelayerId,

        /// The token address if you want an erc20/721 balance
        #[arg(long)]
        token: Option<EvmAddress>,
    },
    /// Manage API keys for relayer access
    ApiKey {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: api_key::ApiKeyCommand,
    },
    /// Manage allowlist addresses for restricted access
    Allowlist {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: allowlist::AllowlistCommand,
    },
    /// Create a new relayer client instance
    Create {
        #[clap(long, short)]
        path: Option<String>,

        /// The relayer name
        #[clap(required = true)]
        name: String,

        /// Network to assign it to
        #[arg(required = true)]
        network: String,
    },
    /// Sign messages and transactions
    Sign {
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: sign::SignCommand,
    },
    /// Manage and monitor transactions
    Tx {
        #[clap(long, short)]
        path: Option<String>,

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
