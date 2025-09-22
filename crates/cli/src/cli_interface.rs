use clap::{Parser, Subcommand};
use rrelayer_core::{common_types::EvmAddress, relayer::types::RelayerId};

use crate::commands::{allowlist, auth::AuthCommand, config, network::NetworkCommands, sign, tx};

/// Main CLI structure for the rrelayer command-line interface.
///
/// This struct defines the top-level CLI parser using clap, which handles
/// command-line argument parsing and subcommand routing.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Enumeration of all available CLI commands.
///
/// Each variant represents a different subcommand that can be executed,
/// with their respective arguments and options defined inline.
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

        /// Number of results to return (default: 10)
        #[clap(long, default_value = "10")]
        limit: u32,

        /// Number of results to skip (default: 0)
        #[clap(long, default_value = "0")]
        offset: u32,
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
}
