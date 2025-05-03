use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{http::HeaderValue, Router};
use dotenv::dotenv;
use thiserror::Error;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info};

use crate::{
    app_state::AppState,
    authentication::{api::create_authentication_routes, types::JwtRole},
    gas::{
        api::create_gas_routes,
        blob_gas_oracle::{blob_gas_oracle, BlobGasOracleCache},
        gas_oracle::{gas_oracle, GasOracleCache},
    },
    keystore::{recover_wallet_from_keystore, KeyStorePasswordManager, PasswordError},
    network::api::create_network_routes,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{load_providers, EvmProvider, LoadProvidersError},
    relayer::api::create_relayer_routes,
    schema::apply_schema,
    setup::yaml::{read, ReadYamlError},
    setup_info_logger,
    shared::{cache::Cache, common_types::EvmAddress},
    transaction::{
        api::create_transactions_routes,
        queue_system::{
            startup_transactions_queues, transactions_queues::TransactionsQueues,
            StartTransactionsQueuesError,
        },
    },
    user::api::create_user_routes,
    AdminIdentifier,
};

fn start_crons(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
) {
    info!("Running cron task...");
    tokio::spawn(gas_oracle(Arc::clone(&providers), gas_oracle_cache));
    tokio::spawn(blob_gas_oracle(providers, blob_gas_oracle_cache));
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum StartApiError {
    #[error("Failed to connect to the database: {0}")]
    DatabaseConnectionError(PostgresConnectionError),

    #[error("Failed to save to the database: {0}")]
    DatabaseSaveError(#[from] PostgresError),

    #[error("Failed to start the API: {0}")]
    ApiStartupError(#[from] std::io::Error),
}

async fn start_api(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    allowed_origins: Option<Vec<String>>,
) -> Result<(), StartApiError> {
    let mut db = PostgresClient::new().await.map_err(StartApiError::DatabaseConnectionError)?;

    for provider in providers.as_ref() {
        db.save_enabled_network(&provider.chain_id, &provider.name, &provider.provider_urls)
            .await?;
    }

    let app_state = Arc::new(AppState {
        db: Arc::new(db),
        evm_providers: providers,
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transactions_queues,
        cache,
    });

    let cors = CorsLayer::new()
        .allow_origin(if allowed_origins.as_ref().map_or(true, |origins| origins.is_empty()) {
            AllowOrigin::any()
        } else {
            AllowOrigin::list(
                allowed_origins
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|origin| HeaderValue::from_str(&origin).ok())
                    .collect::<Vec<HeaderValue>>(),
            )
        })
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .nest("/authentication", create_authentication_routes())
        .nest("/gas", create_gas_routes())
        .nest("/networks", create_network_routes())
        .nest("/relayers", create_relayer_routes())
        .nest("/transactions", create_transactions_routes())
        .nest("/users", create_user_routes())
        // .layer(from_fn(auth_middleware)) // TODO: add auth middleware
        .layer(cors)
        .with_state(app_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let address = "localhost:8000".to_string();

    let listener = tokio::net::TcpListener::bind(&address).await?;
    info!("listening on http://{}", address);
    axum::serve(listener, app).await.map_err(StartApiError::ApiStartupError)?;

    Ok(())
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum StartError {
    #[error("Failed to find the yaml file")]
    NoYamlFileFound,

    #[error("{0}")]
    ReadYamlError(#[from] ReadYamlError),

    #[error("Failed to start the API: {0}")]
    ApiStartupError(#[from] StartApiError),

    #[error("{0}")]
    LoadProvidersError(#[from] LoadProvidersError),

    #[error("Failed to start the transactions queues: {0}")]
    StartTransactionsQueuesError(#[from] StartTransactionsQueuesError),

    #[error("Failed to connect to the database: {0}")]
    DatabaseConnectionError(#[from] PostgresConnectionError),

    #[error("Could not add admins to database: {0}")]
    CouldNotAddAdmins(#[from] PostgresError),

    #[error("Could not load keystore admin: {0} - make sure you have logged in with that account")]
    CouldNotLoadKeystoreAdmin(String),
}

pub async fn start(project_path: &PathBuf) -> Result<(), StartError> {
    setup_info_logger();
    dotenv().ok();

    info!("Starting up the server");

    let yaml_path = project_path.join("rrelayerr.yaml");
    if !yaml_path.exists() {
        error!("Found rrelayerr.yaml in the current directory");
        return Err(StartError::NoYamlFileFound);
    }

    let postgres = PostgresClient::new().await?;

    apply_schema(&postgres).await?;
    info!("Applied database schema");

    let config = read(&yaml_path)?;

    let password_manager = KeyStorePasswordManager::new(&config.name);
    let mut admins: Vec<(EvmAddress, JwtRole)> = vec![];
    for admin in config.admins.iter() {
        match admin {
            AdminIdentifier::Name(account) => {
                match password_manager.load(account) {
                    Ok(password) => {
                        let signer = recover_wallet_from_keystore(
                            &project_path.join("keystores").join("accounts").join(account),
                            &password,
                        )
                            .expect("Failed to recover wallet");
                        let address: EvmAddress = signer.address().into();
                        admins.push((address, JwtRole::Admin))
                    }
                    Err(_) => {
                        return Err(StartError::CouldNotLoadKeystoreAdmin(account.to_string()))
                    }
                }
                if !password_manager.load(account).is_ok() {
                    return Err(StartError::CouldNotLoadKeystoreAdmin(account.to_string()));
                }
            }
            AdminIdentifier::EvmAddress(address) => admins.push((address.clone(), JwtRole::Admin)),
        }
    }

    postgres.add_users(&admins).await?;
    info!("Added admin users to database");

    let cache = Arc::new(Cache::new().await);

    let providers = Arc::new(load_providers(&project_path, &config).await?);

    let gas_oracle_cache = Arc::new(Mutex::new(GasOracleCache::new()));
    let blob_gas_oracle_cache = Arc::new(Mutex::new(BlobGasOracleCache::new()));

    let transaction_queue = startup_transactions_queues(
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        cache.clone(),
    )
        .await?;

    start_crons(gas_oracle_cache.clone(), blob_gas_oracle_cache.clone(), providers.clone());

    start_api(
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transaction_queue,
        providers,
        cache,
        config.allowed_origins,
    )
        .await?;

    Ok(())
}
