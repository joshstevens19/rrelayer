use crate::app_state::RelayersInternalOnly;
use crate::authentication::{create_basic_auth_routes, inject_basic_auth_status};
use crate::background_tasks::run_background_tasks;
use crate::common_types::EvmAddress;
use crate::gas::{BlobGasOracleCache, GasOracleCache};
use crate::network::{create_network_routes, ChainId};
use crate::shared::HttpError;
use crate::webhooks::WebhookManager;
use crate::yaml::{ApiKey, NetworkPermissionsConfig, ReadYamlError};
use crate::{
    app_state::AppState,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{load_providers, EvmProvider, LoadProvidersError},
    rate_limiting::RateLimiter,
    read,
    relayer::create_relayer_routes,
    safe_proxy::SafeProxyManager,
    schema::apply_schema,
    setup_info_logger,
    shared::cache::Cache,
    signing::create_signing_routes,
    transaction::{
        api::create_transactions_routes,
        queue_system::{
            startup_transactions_queues, StartTransactionsQueuesError, TransactionsQueues,
        },
    },
    ApiConfig, RateLimitConfig, SafeProxyConfig, SetupConfig,
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::Response,
    routing::get,
    Json, Router,
};
use dotenv::dotenv;
use std::path::Path;
use std::{net::SocketAddr, sync::Arc, time::Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info};

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

/// Health check endpoint
async fn health_check() -> Result<Json<String>, HttpError> {
    Ok(Json("healthy".to_string()))
}

/// Middleware that logs all HTTP requests and responses with timing information.
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
                    error!("{} {} responded with {} after {:?}", method, uri, status, duration);
                } else {
                    error!("{} {} responded with {} after {:?}", method, uri, status, duration);
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
                        let json_str = json.to_string();
                        if json_str.len() > 500 {
                            format!("Response: {}...", &json_str[0..500])
                        } else {
                            format!("Response: {}", json_str)
                        }
                    }
                }
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
            error!("{} {} responded with {} after {:?}", method, uri, status, duration);

            if !error_details.is_empty() {
                error!("Error details: {}", error_details);
            }

            if status == StatusCode::BAD_REQUEST {
                error!("Bad request error: URI={}, method={}", uri, method);
            }
        } else if status.is_server_error() {
            error!("{} {} responded with {} after {:?}", method, uri, status, duration);

            if !error_details.is_empty() {
                error!("Error details: {}", error_details);
            }
        }

        Ok(response)
    } else {
        info!("{} {} responded with {} after {:?}", method, uri, status, duration);
        Ok(response)
    }
}

#[allow(clippy::too_many_arguments)]
async fn start_api(
    api_config: ApiConfig,
    rate_limit_config: Option<RateLimitConfig>,
    network_permissions: Vec<(ChainId, Vec<NetworkPermissionsConfig>)>,
    api_keys: Vec<(ChainId, Vec<ApiKey>)>,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    transactions_queues: Arc<Mutex<TransactionsQueues>>,
    providers: Arc<Vec<EvmProvider>>,
    cache: Arc<Cache>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
    user_rate_limiter: Option<Arc<RateLimiter>>,
    db: Arc<PostgresClient>,
    safe_proxy_manager: Arc<SafeProxyManager>,
    relayer_internal_only: RelayersInternalOnly,
    config: &SetupConfig,
) -> Result<(), StartApiError> {
    // Calculate which networks are configured with only private keys
    let private_key_only_networks: Vec<ChainId> = config
        .networks
        .iter()
        .filter_map(|network_config| {
            // Determine which signing provider to use (network-level or global)
            let signing_provider = if let Some(ref signing_key) = network_config.signing_provider {
                signing_key
            } else if let Some(ref signing_key) = config.signing_provider {
                signing_key
            } else {
                return None;
            };

            // Check if only private keys are configured
            if signing_provider.private_keys.is_some()
                && signing_provider.raw.is_none()
                && signing_provider.aws_secret_manager.is_none()
                && signing_provider.gcp_secret_manager.is_none()
                && signing_provider.privy.is_none()
                && signing_provider.aws_kms.is_none()
                && signing_provider.turnkey.is_none()
                && signing_provider.pkcs11.is_none()
                && signing_provider.fireblocks.is_none()
            {
                Some(network_config.chain_id)
            } else {
                None
            }
        })
        .collect();

    let app_state = Arc::new(AppState {
        db: db.clone(),
        evm_providers: providers,
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transactions_queues,
        cache,
        webhook_manager,
        user_rate_limiter,
        rate_limit_config,
        relayer_creation_mutex: Arc::new(Mutex::new(())),
        safe_proxy_manager,
        relayer_internal_only: Arc::new(relayer_internal_only),
        network_permissions: Arc::new(network_permissions),
        api_keys: Arc::new(api_keys),
        network_configs: Arc::new(config.networks.clone()),
        private_key_only_networks: Arc::new(private_key_only_networks),
    });

    let cors = CorsLayer::new()
        .allow_origin(
            if api_config.allowed_origins.as_ref().is_none_or(|origins| origins.is_empty()) {
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

    // All routes handle their own authentication logic internally
    let api_routes = Router::new()
        .nest("/auth", create_basic_auth_routes())
        .nest("/networks", create_network_routes())
        .nest("/relayers", create_relayer_routes())
        .nest("/transactions", create_transactions_routes())
        .nest("/signing", create_signing_routes());

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(api_routes)
        .layer(middleware::from_fn(inject_basic_auth_status))
        .layer(middleware::from_fn(activity_logger))
        .layer(cors)
        .with_state(app_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let address =
        format!("{}:{}", api_config.host.unwrap_or("localhost".to_string()), api_config.port);

    let listener = tokio::net::TcpListener::bind(&address).await?;
    info!("rrelayer is up on http://{}", address);
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

    #[error("To run rrelayer you need to define at least one network in the yaml file")]
    NoNetworksDefinedInYaml,
}

pub async fn start(project_path: &Path) -> Result<(), StartError> {
    setup_info_logger();
    dotenv().ok();

    info!("Starting up the server");

    let yaml_path = project_path.join("rrelayer.yaml");
    if !yaml_path.exists() {
        error!("Found rrelayer.yaml in the current directory");
        return Err(StartError::NoYamlFileFound);
    }

    let config = read(&yaml_path, false)?;

    if config.networks.is_empty() {
        return Err(StartError::NoNetworksDefinedInYaml);
    }

    let postgres = PostgresClient::new().await?;

    apply_schema(&postgres).await?;
    info!("Applied database schema");

    let cache = Arc::new(Cache::new().await);

    let providers = Arc::new(load_providers(project_path, &config).await?);

    let gas_oracle_cache = Arc::new(Mutex::new(GasOracleCache::new()));
    let blob_gas_oracle_cache = Arc::new(Mutex::new(BlobGasOracleCache::new()));

    let postgres_client = Arc::new(postgres);

    let webhook_manager = if config.webhooks.is_some() {
        info!("Initializing webhook manager with configuration");
        Some(Arc::new(Mutex::new(WebhookManager::new(
            Arc::clone(&postgres_client),
            &config,
            None,
        )?)))
    } else {
        info!("Webhooks disabled - no webhook configuration found");
        None
    };

    let mut safe_configs: Vec<SafeProxyConfig> = vec![];
    let mut relayer_internal_only: Vec<(ChainId, EvmAddress)> = vec![];
    let mut network_permissions: Vec<(ChainId, Vec<NetworkPermissionsConfig>)> = vec![];
    let mut api_keys: Vec<(ChainId, Vec<ApiKey>)> = vec![];
    for network_config in &config.networks {
        api_keys
            .push((network_config.chain_id, network_config.api_keys.clone().unwrap_or_default()));
        if let Some(automatic_top_up_configs) = &network_config.automatic_top_up {
            for automatic_top_up in automatic_top_up_configs {
                if let Some(safe_address) = &automatic_top_up.from.safe {
                    safe_configs.push(SafeProxyConfig {
                        address: *safe_address,
                        relayers: vec![automatic_top_up.from.relayer.address],
                        chain_id: network_config.chain_id,
                    })
                }
                if automatic_top_up.from.relayer.internal_only.unwrap_or(true) {
                    relayer_internal_only
                        .push((network_config.chain_id, automatic_top_up.from.relayer.address))
                }
            }
        }

        if let Some(permissions) = &network_config.permissions {
            network_permissions.push((network_config.chain_id, permissions.clone()))
        }
    }

    let safe_proxy_manager = Arc::new(SafeProxyManager::new(safe_configs));
    let relayer_internal_only = RelayersInternalOnly::new(relayer_internal_only);

    let transaction_queue = startup_transactions_queues(
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        cache.clone(),
        webhook_manager.clone(),
        safe_proxy_manager.clone(),
        Arc::new(config.networks.clone()),
        config.signing_provider.clone().map(Arc::new),
    )
    .await?;

    run_background_tasks(
        &config,
        gas_oracle_cache.clone(),
        blob_gas_oracle_cache.clone(),
        providers.clone(),
        postgres_client.clone(),
        webhook_manager.clone(),
        transaction_queue.clone(),
        safe_proxy_manager.clone(),
    )
    .await;

    let user_rate_limiter = if let Some(ref rate_limit_config) = config.rate_limits {
        info!("Initializing user rate limiter with configuration");
        let user_rate_limiter = RateLimiter::new(rate_limit_config.clone());

        info!("User rate limiter initialized successfully");
        Some(Arc::new(user_rate_limiter))
    } else {
        info!("Rate limiting disabled - no configuration found");
        None
    };

    start_api(
        config.api_config.clone(),
        config.rate_limits.clone(),
        network_permissions,
        api_keys,
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transaction_queue,
        providers,
        cache,
        webhook_manager,
        user_rate_limiter,
        postgres_client,
        safe_proxy_manager,
        relayer_internal_only,
        &config,
    )
    .await?;

    Ok(())
}
