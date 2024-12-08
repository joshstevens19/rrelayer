use std::{env, net::SocketAddr, sync::Arc};

use axum::{http::HeaderValue, Router};
use dotenv::dotenv;
use thiserror::Error;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::info;

use crate::{
    app_state::AppState,
    authentication::{api::create_authentication_routes, types::JwtRole},
    gas::{
        api::create_gas_routes,
        gas_oracle::{gas_oracle, GasOracleCache},
    },
    network::api::create_network_routes,
    postgres::{PostgresClient, PostgresConnectionError},
    provider::{load_providers, EvmProvider, LoadProvidersError},
    relayer::api::create_relayer_routes,
    setup::yaml::{read, ReadYamlError},
    shared::{cache::Cache, common_types::EvmAddress},
    transaction::{
        api::create_transactions_routes,
        queue_system::{
            startup_transactions_queues, transactions_queues::TransactionsQueues,
            StartTransactionsQueuesError,
        },
    },
    user::api::create_user_routes,
};

fn start_crons(gas_oracle_cache: Arc<Mutex<GasOracleCache>>, providers: Arc<Vec<EvmProvider>>) {
    info!("Running cron task...");
    tokio::spawn(gas_oracle(providers, gas_oracle_cache));
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum StartApiError {
    #[error("Failed to connect to the database: {0}")]
    DatabaseConnectionError(PostgresConnectionError),

    #[error("Failed to save to the database: {0}")]
    DatabaseSaveError(tokio_postgres::Error),

    #[error("Failed to start the API: {0}")]
    ApiStartupError(#[from] std::io::Error),
}

async fn start_api(
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    allowed_origins: Option<Vec<String>>,
) -> Result<(), StartApiError> {
    let mut db = PostgresClient::new().await.map_err(StartApiError::DatabaseConnectionError)?;

    // save providers to database
    for provider in providers.as_ref() {
        db.save_enabled_network(&provider.chain_id, &provider.name, &provider.provider_urls)
            .await
            .map_err(StartApiError::DatabaseSaveError)?;
    }

    let app_state = Arc::new(AppState {
        db: Arc::new(db),
        evm_providers: providers,
        gas_oracle_cache,
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
        // .layer(from_fn(auth_middleware))
        .layer(cors)
        .with_state(app_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let address = "localhost:8000".to_string();

    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();
    info!("listening on http://{}", address);
    axum::serve(listener, app).await.map_err(StartApiError::ApiStartupError)?;

    Ok(())
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum StartError {
    #[error("{0}")]
    ReadYamlError(ReadYamlError),

    #[error("Failed to start the API: {0}")]
    ApiStartupError(StartApiError),

    #[error("{0}")]
    LoadProvidersError(LoadProvidersError),

    #[error("Failed to start the transactions queues: {0}")]
    StartTransactionsQueuesError(StartTransactionsQueuesError),

    #[error("Failed to connect to the database: {0}")]
    DatabaseConnectionError(PostgresConnectionError),

    #[error("Could not add admins to database: {0}")]
    CouldNotAddAdmins(tokio_postgres::Error),
}

pub async fn start() -> Result<(), StartError> {
    dotenv().ok();

    let mut path = env::current_dir().unwrap();
    path.push("setup.yaml");

    let config = read(&path).map_err(StartError::ReadYamlError)?;

    let postgres = PostgresClient::new().await.map_err(StartError::DatabaseConnectionError)?;

    let admins: Vec<(&EvmAddress, JwtRole)> =
        config.admins.iter().map(|address| (address, JwtRole::Admin)).collect();

    postgres.add_users(&admins).await.map_err(StartError::CouldNotAddAdmins)?;

    let cache = Arc::new(Cache::new().await);

    let providers = load_providers(&config).await;
    match providers {
        Ok(providers) => {
            let providers = Arc::new(providers);

            let gas_oracle_cache = Arc::new(Mutex::new(GasOracleCache::new()));

            let transaction_queue = startup_transactions_queues(
                gas_oracle_cache.clone(),
                providers.clone(),
                cache.clone(),
            )
            .await
            .map_err(StartError::StartTransactionsQueuesError)?;

            start_crons(gas_oracle_cache.clone(), providers.clone());

            start_api(
                gas_oracle_cache,
                transaction_queue,
                providers,
                cache,
                config.allowed_origins,
            )
            .await
            .map_err(StartError::ApiStartupError)?;

            Ok(())
        }
        Err(e) => Err(StartError::LoadProvidersError(e)),
    }
}
