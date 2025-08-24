use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

use axum::{
    body::{to_bytes, Body},
    http::{HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use dotenv::dotenv;
use thiserror::Error;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info};

use crate::background_tasks::run_background_tasks;
use crate::keystore::{recover_wallet_from_keystore, KeyStorePasswordManager};
use crate::yaml::ReadYamlError;
use crate::{
    app_state::AppState,
    authentication::{api::create_authentication_routes, types::JwtRole},
    gas::{
        api::create_gas_routes, blob_gas_oracle::BlobGasOracleCache, gas_oracle::GasOracleCache,
    },
    network::api::create_network_routes,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{load_providers, EvmProvider, LoadProvidersError},
    read,
    relayer::api::create_relayer_routes,
    rrelayer_error, rrelayer_info,
    schema::apply_schema,
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
    AdminIdentifier, ApiConfig,
};

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

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

async fn activity_logger(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status();
    let duration = start.elapsed();

    if status.is_client_error() || status.is_server_error() {
        let (parts, body) = response.into_parts();

        let bytes = match to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(_) => {
                if status.is_client_error() {
                    rrelayer_error!(
                        "{} {} responded with {} after {:?}",
                        method,
                        uri,
                        status,
                        duration
                    );
                } else {
                    rrelayer_error!(
                        "{} {} responded with {} after {:?}",
                        method,
                        uri,
                        status,
                        duration
                    );
                }
                return Ok(Response::builder().status(status).body(Body::empty()).unwrap());
            }
        };

        let error_details = if !bytes.is_empty() {
            match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(json) => {
                    if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                        format!("Error: {}", error)
                    } else if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
                        format!("Message: {}", message)
                    } else {
                        // Just grab a preview of the JSON
                        let json_str = json.to_string();
                        if json_str.len() > 500 {
                            format!("Response: {}...", &json_str[0..500])
                        } else {
                            format!("Response: {}", json_str)
                        }
                    }
                }
                // If not JSON, try to display as string if it's UTF-8
                Err(_) => match std::str::from_utf8(&bytes) {
                    Ok(s) if !s.trim().is_empty() => {
                        if s.len() > 500 {
                            format!("Response: {}...", &s[0..500])
                        } else {
                            format!("Response: {}", s)
                        }
                    }
                    _ => "".to_string(),
                },
            }
        } else {
            "".to_string()
        };

        let response = Response::from_parts(parts, Body::from(bytes));

        if status.is_client_error() {
            rrelayer_error!("{} {} responded with {} after {:?}", method, uri, status, duration);

            if !error_details.is_empty() {
                rrelayer_error!("Error details: {}", error_details);
            }

            if status == StatusCode::BAD_REQUEST {
                rrelayer_error!("Bad request error: URI={}, method={}", uri, method);
            }
        } else if status.is_server_error() {
            rrelayer_error!("{} {} responded with {} after {:?}", method, uri, status, duration);

            if !error_details.is_empty() {
                rrelayer_error!("Error details: {}", error_details);
            }
        }

        Ok(response)
    } else {
        rrelayer_info!("{} {} responded with {} after {:?}", method, uri, status, duration);
        Ok(response)
    }
}

async fn start_api(
    api_config: ApiConfig,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
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
        .allow_origin(
            if api_config.allowed_origins.as_ref().map_or(true, |origins| origins.is_empty()) {
                AllowOrigin::any()
            } else {
                AllowOrigin::list(
                    api_config
                        .allowed_origins
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|origin| HeaderValue::from_str(&origin).ok())
                        .collect::<Vec<HeaderValue>>(),
                )
            },
        )
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/authentication", create_authentication_routes())
        .nest("/gas", create_gas_routes())
        .nest("/networks", create_network_routes())
        .nest("/relayers", create_relayer_routes())
        .nest("/transactions", create_transactions_routes())
        .nest("/users", create_user_routes())
        .layer(middleware::from_fn(activity_logger))
        // .layer(from_fn(auth_middleware)) // TODO: add auth middleware
        .layer(cors)
        .with_state(app_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let address = format!("localhost:{}", api_config.port);

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

    let yaml_path = project_path.join("rrelayer.yaml");
    if !yaml_path.exists() {
        error!("Found rrelayer.yaml in the current directory");
        return Err(StartError::NoYamlFileFound);
    }

    let postgres = PostgresClient::new().await?;

    apply_schema(&postgres).await?;
    info!("Applied database schema");

    let config = read(&yaml_path, false)?;

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

    run_background_tasks(
        &config,
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        Arc::new(postgres),
    )
    .await;

    let transaction_queue = startup_transactions_queues(
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        cache.clone(),
    )
    .await?;

    start_api(
        config.api_config,
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transaction_queue,
        providers,
        cache,
    )
    .await?;

    Ok(())
}
