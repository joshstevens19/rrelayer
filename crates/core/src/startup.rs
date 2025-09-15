use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

use crate::authentication::api::create_basic_auth_routes;
use crate::background_tasks::run_background_tasks;
use crate::yaml::ReadYamlError;
use crate::{
    app_state::AppState,
    authentication::guards::basic_auth_guard,
    gas::{
        api::create_gas_routes, blob_gas_oracle::BlobGasOracleCache, gas_oracle::GasOracleCache,
    },
    network::api::create_network_routes,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{load_providers, EvmProvider, LoadProvidersError},
    read,
    relayer::api::create_relayer_routes,
    rrelayer_error, rrelayer_info,
    safe_proxy::SafeProxyManager,
    schema::apply_schema,
    setup_info_logger,
    shared::cache::Cache,
    signing::create_signing_history_routes,
    transaction::{
        api::create_transactions_routes,
        queue_system::{
            startup_transactions_queues, transactions_queues::TransactionsQueues,
            StartTransactionsQueuesError,
        },
    },
    user_rate_limiting::UserRateLimiter,
    ApiConfig, RateLimitConfig,
};
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
use tracing::error;
use tracing::log::info;

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

/// Health check endpoint that returns HTTP 200 OK.
///
/// Used by load balancers and monitoring systems to verify the service is running.
async fn health_check() -> impl IntoResponse {
    "healthy"
}

/// Middleware that logs all HTTP requests and responses with timing information.
///
/// Provides detailed logging for client and server errors, including response body
/// content for debugging purposes. For successful requests, logs basic timing info.
///
/// # Arguments
/// * `req` - The incoming HTTP request
/// * `next` - The next middleware or handler in the chain
///
/// # Returns
/// * `Ok(Response)` - The response from the downstream handler
/// * `Err(StatusCode)` - Internal server error if response processing fails
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
                return match Response::builder().status(status).body(Body::empty()) {
                    Ok(response) => Ok(response),
                    Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
                };
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

/// Starts the HTTP API server with all configured routes and middleware.
///
/// Sets up the Axum web server with CORS, logging middleware, and all API routes.
/// Initializes the application state with database connections, caches, and providers.
///
/// # Arguments
/// * `api_config` - API configuration including port and allowed origins
/// * `gas_oracle_cache` - Shared cache for gas price estimations
/// * `blob_gas_oracle_cache` - Shared cache for blob gas prices
/// * `transactions_queues` - Transaction processing queues
/// * `providers` - EVM provider connections
/// * `cache` - General purpose cache
/// * `webhook_manager` - Webhook delivery manager
///
/// # Returns
/// * `Ok(())` - If the server starts successfully
/// * `Err(StartApiError)` - If server startup fails
async fn start_api(
    api_config: ApiConfig,
    rate_limit_config: Option<RateLimitConfig>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<crate::webhooks::WebhookManager>>>,
    user_rate_limiter: Option<Arc<UserRateLimiter>>,
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
        webhook_manager,
        user_rate_limiter,
        rate_limit_config,
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

    let protected_routes = Router::new()
        .nest("/auth", create_basic_auth_routes())
        .nest("/gas", create_gas_routes())
        .nest("/networks", create_network_routes())
        .nest("/relayers", create_relayer_routes())
        .nest("/transactions", create_transactions_routes())
        .nest("/signing", create_signing_history_routes())
        .layer(middleware::from_fn(basic_auth_guard));

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
        .layer(middleware::from_fn(activity_logger))
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

    #[error("Could not apply db schema to postgres: {0}")]
    CouldNotApplyDbSchema(#[from] PostgresError),

    #[error("Could not load keystore admin: {0} - make sure you have logged in with that account")]
    CouldNotLoadKeystoreAdmin(String),

    #[error("Webhook manager creation error: {0}")]
    WebhookManagerError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Starts the RRelayer service with full initialization.
///
/// This is the main entry point that:
/// 1. Sets up logging
/// 2. Loads configuration from rrelayer.yaml
/// 3. Initializes database connection and applies schema
/// 4. Sets up admin users and authentication
/// 5. Initializes blockchain providers and caches
/// 6. Starts background tasks for gas estimation and transaction processing
/// 7. Starts the HTTP API server
///
/// # Arguments
/// * `project_path` - Path to the project directory containing rrelayer.yaml
///
/// # Returns
/// * `Ok(())` - If the service starts successfully
/// * `Err(StartError)` - If any initialization step fails
///
/// # Example
/// ```rust,no_run
/// use std::path::PathBuf;
/// use rrelayer_core::start;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let project_path = PathBuf::from(".");
///     start(&project_path).await?;
///     Ok(())
/// }
/// ```
pub async fn start(project_path: &PathBuf) -> Result<(), StartError> {
    setup_info_logger();
    dotenv().ok();

    info!("Starting up the server");

    let yaml_path = project_path.join("rrelayer.yaml");
    if !yaml_path.exists() {
        rrelayer_error!("Found rrelayer.yaml in the current directory");
        return Err(StartError::NoYamlFileFound);
    }

    let postgres = PostgresClient::new().await?;

    apply_schema(&postgres).await?;
    info!("Applied database schema");

    let config = read(&yaml_path, false)?;

    let safe_proxy_manager = if let Some(ref safe_proxy_configs) = config.safe_proxy {
        if !safe_proxy_configs.is_empty() {
            rrelayer_info!(
                "Initializing safe proxy with {} configurations",
                safe_proxy_configs.len()
            );
            Some(SafeProxyManager::new(safe_proxy_configs.clone()))
        } else {
            None
        }
    } else {
        None
    };

    let cache = Arc::new(Cache::new().await);

    let providers = Arc::new(load_providers(&project_path, &config).await?);

    let gas_oracle_cache = Arc::new(Mutex::new(GasOracleCache::new()));
    let blob_gas_oracle_cache = Arc::new(Mutex::new(BlobGasOracleCache::new()));

    let postgres_client = Arc::new(postgres);

    let webhook_manager = if config.webhooks.is_some() {
        rrelayer_info!("Initializing webhook manager with configuration");
        Some(Arc::new(Mutex::new(crate::webhooks::WebhookManager::new(&config, None)?)))
    } else {
        rrelayer_info!("Webhooks disabled - no webhook configuration found");
        None
    };

    let transaction_queue = startup_transactions_queues(
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        cache.clone(),
        webhook_manager.clone(),
        safe_proxy_manager,
    )
    .await?;

    let user_rate_limiter = if let Some(ref rate_limit_config) = config.user_rate_limits {
        rrelayer_info!("Initializing user rate limiter with configuration");
        let user_rate_limiter =
            UserRateLimiter::new(rate_limit_config.clone(), postgres_client.clone());

        if let Err(e) = user_rate_limiter.initialize().await {
            rrelayer_error!("Failed to initialize user rate limiter: {}", e);
            None
        } else {
            rrelayer_info!("User rate limiter initialized successfully");
            Some(Arc::new(user_rate_limiter))
        }
    } else {
        rrelayer_info!("Rate limiting disabled - no configuration found");
        None
    };

    run_background_tasks(
        &config,
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        postgres_client.clone(),
        user_rate_limiter.clone(),
        webhook_manager.clone(),
    )
    .await;

    start_api(
        config.api_config,
        config.user_rate_limits.clone(),
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transaction_queue,
        providers,
        cache,
        webhook_manager,
        user_rate_limiter,
    )
    .await?;

    Ok(())
}
