use clap::{Parser, Subcommand};
use rrelayer_core::{common_types::EvmAddress, relayer::RelayerId};

use crate::commands::{allowlist, auth::AuthCommand, config, network::NetworkCommands, sign, tx};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new rrelayer project
    New {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,
    },
    /// Clone an existing relayer private key to another network
    Clone {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        /// The unique identifier of the relayer to clone from
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        /// The new relayer name
        #[arg(long, short = 'n')]
        name: String,

        /// Network to assign it to
        #[arg(long)]
        network: String,
    },
    /// Authenticate with rrelayer
    Auth {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[clap(subcommand)]
        command: AuthCommand,
    },
    /// Start the relayer service
    Start {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,
    },
    /// Manage network configurations and settings
    Network {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: NetworkCommands,
    },
    /// List all configured relayers
    List {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        /// Network to get the list back
        #[arg(long, short)]
        network: Option<String>,

        /// Number of results to return (default: 10)
        #[clap(long, default_value = "10")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[clap(long, default_value = "0")]
        offset: u32,
    },
    /// Configure operations for a specific relayer
    Config {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    /// Check the balance of a relayer's account
    Balance {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        /// The unique identifier of the relayer
        #[arg(long, short = 'r')]
        relayer_id: RelayerId,

        /// The token address if you want an erc20/721 balance
        #[arg(long)]
        token: Option<EvmAddress>,
    },
    /// Manage allowlist addresses for restricted access
    Allowlist {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: allowlist::AllowlistCommand,
    },
    /// Create a new relayer client instance
    Create {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        /// The relayer name
        #[arg(long, short = 'n')]
        name: String,

        /// Network to assign it to
        #[arg(long)]
        network: String,
    },
    /// Sign messages and transactions
    Sign {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: sign::SignCommand,
    },
    /// Manage and monitor transactions
    Tx {
        /// optional - The path to create the project in, default will be where the command is run.
        #[clap(long, short)]
        path: Option<String>,

        #[command(subcommand)]
        command: tx::TxCommand,
    },
}
