//! RRelayer Core Library
//!
//! This is the core library for RRelayer, a blockchain transaction relaying service
//! that provides secure, scalable, and efficient transaction relaying capabilities.
//!
//! The library provides modules for:
//! - Authentication and user management
//! - Gas fee estimation and management
//! - Network configuration and provider management
//! - PostgreSQL database integration
//! - Transaction queuing and processing
//! - Wallet management and signing
//! - Background task processing
//! - Webhook integration
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use std::path::PathBuf;
//! use rrelayer_core::{start, StartError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), StartError> {
//!     let project_path = PathBuf::from(".");
//!     start(&project_path).await
//! }
//! ```

mod app_state;
pub mod authentication;
pub mod gas;
mod logger;
pub use logger::setup_info_logger;
mod middleware;
pub mod network;
mod postgres;
pub use postgres::{PostgresClient, PostgresConnectionError};
mod provider;
pub use provider::create_retry_client;
pub mod relayer;
pub mod safe_proxy;
pub use safe_proxy::{SafeProxyError, SafeProxyManager, SafeTransaction};
pub use yaml::{
    read, AdminIdentifier, ApiConfig, AwsKmsSigningKey, GasProviders, KeystoreSigningKey, KmsKeyIds, NetworkSetupConfig,
    SafeProxyConfig, SetupConfig, SigningKey,
};
mod shared;
pub use shared::common_types;
mod startup;
pub use startup::{start, StartError};
mod docker;
mod environment;
mod file;
mod schema;
pub mod transaction;
pub mod user;
mod wallet;
pub use wallet::{generate_seed_phrase, keystore, AwsKmsWalletManager, WalletError};
mod background_tasks;
mod webhooks;
mod yaml;

pub use docker::generate_docker_file;
pub use environment::load_env_from_project_path;
pub use file::{write_file, WriteFileError};
pub use tracing::{error as rrelayer_error, info as rrelayer_info};
