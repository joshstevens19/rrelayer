use crate::app_state::{RelayersAllowedForRandom, RelayersInternalOnly};
use crate::authentication::{create_basic_auth_routes, inject_basic_auth_status};
use crate::background_tasks::run_background_tasks;
use crate::common_types::{EvmAddress, PagingContext, PagingResult};
use crate::gas::{BlobGasOracleCache, GasOracleCache};
use crate::network::{create_network_routes, ChainId};
use crate::rate_limiting::RATE_LIMIT_HEADER_NAME;
use crate::shared::{bad_request, not_found, HttpError};
use crate::webhooks::WebhookManager;
use crate::yaml::{AllOrOneOrManyAddresses, ApiKey, NetworkPermissionsConfig, ReadYamlError};
use crate::{
    app_state::AppState,
    postgres::{PostgresClient, PostgresConnectionError, PostgresError},
    provider::{chain_enabled, find_provider_for_chain_id, load_providers, LoadProvidersError},
    rate_limiting::RateLimiter,
    read,
    relayer::{
        clone_relayer_core, create_relayer_core, create_relayer_routes, get_relayer,
        CreateRelayerResult, GetRelayerResult, RelayerId,
    },
    safe_proxy::SafeProxyManager,
    schema::apply_schema,
    setup_info_logger,
    shared::cache::Cache,
    shutdown,
    signing::create_signing_routes,
    transaction::{
        api::{
            create_transactions_routes, send_transaction, transaction_status_result,
            RelayTransactionRequest, RelayTransactionStatusResult, SendTransactionResult,
        },
        get_transaction_by_id,
        queue_system::{startup_transactions_queues, StartTransactionsQueuesError},
        types::{Transaction, TransactionId},
    },
    ApiConfig, SafeProxyConfig,
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::Response,
    routing::get,
    Json, Router,
};
use dotenv::dotenv;
use rustls::crypto::ring::default_provider;
use rustls::crypto::CryptoProvider;
use std::collections::HashMap;
use std::path::Path;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{error, info, warn};

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

async fn start_api(api_config: ApiConfig, app_state: Arc<AppState>) -> Result<(), StartApiError> {
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

    let shutdown_signal = async {
        let ctrl_c = async {
            tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(windows)]
        let terminate = async {
            tokio::signal::windows::ctrl_break()
                .expect("failed to install Ctrl+Break handler")
                .recv()
                .await;
        };

        #[cfg(not(any(unix, windows)))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received Ctrl+C, initiating graceful shutdown");
            },
            _ = terminate => {
                #[cfg(unix)]
                info!("Received SIGTERM, initiating graceful shutdown");
                #[cfg(windows)]
                info!("Received Ctrl+Break, initiating graceful shutdown");
                #[cfg(not(any(unix, windows)))]
                info!("Received terminate signal, initiating graceful shutdown");
            },
        }
    };

    tokio::select! {
        result = axum::serve(listener, app) => {
            result.map_err(StartApiError::ApiStartupError)?;
        }
        _ = shutdown_signal => {
            info!("Starting graceful shutdown...");

            let shutdown_successful = shutdown::request_graceful_shutdown(Duration::from_secs(30)).await;

            if shutdown_successful {
                info!("Graceful shutdown completed successfully");
            } else {
                warn!("Some operations did not complete within shutdown timeout");
            }
        }
    }

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

/// Builds a full rrelayer instance without binding the HTTP API, returning a [`Relayer`]
/// handle which can either serve the API or be driven in-process by an embedder.
pub async fn build(project_path: &Path) -> Result<Relayer, StartError> {
    setup_info_logger();
    dotenv().ok();

    info!("Starting up the server");

    let yaml_path = project_path.join("rrelayer.yaml");
    info!("Config path {}", yaml_path.display());
    if !yaml_path.exists() {
        error!("Found no rrelayer.yaml in the current directory {}", yaml_path.display());
        return Err(StartError::NoYamlFileFound);
    }

    let config = read(&yaml_path, false)?;
    info!("Config loaded successfully from {}", yaml_path.display());

    if config.networks.is_empty() {
        return Err(StartError::NoNetworksDefinedInYaml);
    }

    let postgres = PostgresClient::new().await?;

    apply_schema(&postgres).await?;
    info!("Applied database schema");

    if CryptoProvider::get_default().is_none() {
        // Ignore the AlreadyInstalled race: another part of the process (e.g. an embedder)
        // may have installed a provider between the check and the install.
        let _ = CryptoProvider::install_default(default_provider());
    }

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
    let mut relayers_allowed_for_random: HashMap<ChainId, Vec<EvmAddress>> = HashMap::new();
    let mut network_permissions: Vec<(ChainId, Vec<NetworkPermissionsConfig>)> = vec![];
    let mut api_keys: Vec<(ChainId, Vec<ApiKey>)> = vec![];
    for network_config in &config.networks {
        api_keys
            .push((network_config.chain_id, network_config.api_keys.clone().unwrap_or_default()));

        if let Some(allowed_random) = &network_config.allowed_random_relayers {
            let allowed_addresses = match allowed_random {
                AllOrOneOrManyAddresses::All => {
                    // Empty vector means all relayers are allowed
                    vec![]
                }
                AllOrOneOrManyAddresses::One(address) => {
                    vec![*address]
                }
                AllOrOneOrManyAddresses::Many(addresses) => addresses.clone(),
            };
            relayers_allowed_for_random.insert(network_config.chain_id, allowed_addresses);
        }

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
    let relayers_allowed_for_random = RelayersAllowedForRandom::new(relayers_allowed_for_random);

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

    // Calculate which networks are configured with only private keys
    let private_key_only_networks: Vec<ChainId> = config
        .networks
        .iter()
        .filter_map(|network_config| {
            // Determine which signing provider to use (network-level or global)
            let signing_provider = if let Some(ref signing_key) = network_config.signing_provider {
                signing_key
            } else {
                config.signing_provider.as_ref()?
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
        db: postgres_client,
        evm_providers: providers,
        gas_oracle_cache,
        blob_gas_oracle_cache,
        transactions_queues: transaction_queue,
        cache,
        webhook_manager,
        user_rate_limiter,
        rate_limit_config: config.rate_limits.clone(),
        relayer_creation_mutex: Arc::new(Mutex::new(())),
        safe_proxy_manager,
        relayer_internal_only: Arc::new(relayer_internal_only),
        relayers_allowed_for_random: Arc::new(relayers_allowed_for_random),
        network_permissions: Arc::new(network_permissions),
        api_keys: Arc::new(api_keys),
        network_configs: Arc::new(config.networks.clone()),
        private_key_only_networks: Arc::new(private_key_only_networks),
    });

    Ok(Relayer { api_config: config.api_config, app_state })
}

pub async fn start(project_path: &Path) -> Result<(), StartError> {
    let relayer = build(project_path).await?;
    relayer.serve_api().await
}

/// A fully built rrelayer instance.
///
/// Created via [`build`], it owns every component the server needs. Call [`Relayer::serve_api`]
/// to serve the HTTP API (what [`start`] does) or use the in-process methods directly when
/// embedding rrelayer inside another application - no port has to be bound for those.
///
/// The in-process methods mirror the corresponding HTTP endpoints with authentication
/// excluded - the embedder is trusted.
pub struct Relayer {
    api_config: ApiConfig,
    app_state: Arc<AppState>,
}

impl Relayer {
    /// Serves the HTTP API until shutdown, exactly as the standalone server does.
    pub async fn serve_api(self) -> Result<(), StartError> {
        start_api(self.api_config, self.app_state).await?;

        Ok(())
    }

    /// Builds headers marking the request as trusted, mirroring what the HTTP
    /// basic auth middleware injects for authenticated requests.
    fn trusted_headers(rate_limit_key: Option<&str>) -> Result<HeaderMap, HttpError> {
        let mut headers = HeaderMap::new();
        headers.insert("x-rrelayer-basic-auth-valid", HeaderValue::from_static("true"));

        if let Some(rate_limit_key) = rate_limit_key {
            let value = HeaderValue::from_str(rate_limit_key)
                .map_err(|_| bad_request("Invalid rate limit key".to_string()))?;
            headers.insert(RATE_LIMIT_HEADER_NAME, value);
        }

        Ok(headers)
    }

    /// Sends a transaction through the relayer's queue - mirrors the send transaction endpoint.
    pub async fn send_transaction(
        &self,
        relayer_id: &RelayerId,
        request: &RelayTransactionRequest,
        rate_limit_key: Option<String>,
    ) -> Result<SendTransactionResult, HttpError> {
        let relayer = get_relayer(&self.app_state.db, &self.app_state.cache, relayer_id)
            .await?
            .ok_or(not_found("Relayer does not exist".to_string()))?;

        let headers = Self::trusted_headers(rate_limit_key.as_deref())?;

        send_transaction(relayer, request.clone(), &self.app_state, &headers).await
    }

    /// Gets the status of a transaction - mirrors the transaction status endpoint.
    pub async fn get_transaction_status(
        &self,
        id: &TransactionId,
    ) -> Result<Option<RelayTransactionStatusResult>, HttpError> {
        let transaction =
            get_transaction_by_id(&self.app_state.cache, &self.app_state.db, *id).await?;

        match transaction {
            None => Ok(None),
            Some(transaction) => {
                Ok(Some(transaction_status_result(&self.app_state, transaction).await?))
            }
        }
    }

    /// Gets the latest transaction for a relayer by its external id.
    pub async fn get_transaction_by_external_id(
        &self,
        relayer_id: &RelayerId,
        external_id: &str,
    ) -> Result<Option<Transaction>, HttpError> {
        let transaction = self
            .app_state
            .db
            .get_transaction_by_relayer_and_external_id(relayer_id, external_id)
            .await?;

        Ok(transaction)
    }

    /// Creates a new relayer for the specified network - mirrors the create relayer endpoint.
    pub async fn create_relayer(
        &self,
        chain_id: u64,
        name: &str,
    ) -> Result<CreateRelayerResult, HttpError> {
        create_relayer_core(&self.app_state, &ChainId::new(chain_id), name).await
    }

    /// Clones an existing relayer to a new network - mirrors the clone relayer endpoint.
    pub async fn clone_relayer(
        &self,
        relayer_id: &RelayerId,
        chain_id: u64,
        name: &str,
    ) -> Result<CreateRelayerResult, HttpError> {
        clone_relayer_core(&self.app_state, relayer_id, &ChainId::new(chain_id), name).await
    }

    /// Gets a relayer with its provider urls - mirrors the get relayer endpoint.
    pub async fn get_relayer(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<Option<GetRelayerResult>, HttpError> {
        let relayer = get_relayer(&self.app_state.db, &self.app_state.cache, relayer_id).await?;

        match relayer {
            None => Ok(None),
            Some(relayer) => {
                let provider =
                    find_provider_for_chain_id(&self.app_state.evm_providers, &relayer.chain_id)
                        .await;
                let provider_urls = provider.map(|p| p.provider_urls.clone()).unwrap_or_default();

                Ok(Some(GetRelayerResult { relayer, provider_urls }))
            }
        }
    }

    /// Gets a paginated list of relayers, optionally filtered by chain id - mirrors the
    /// get relayers endpoint.
    pub async fn get_relayers(
        &self,
        chain_id: Option<u64>,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<crate::relayer::Relayer>, HttpError> {
        match chain_id {
            Some(chain_id) => {
                let chain_id = ChainId::new(chain_id);
                if !chain_enabled(&self.app_state.evm_providers, &chain_id) {
                    return Err(bad_request("Chain is not enabled".to_string()));
                }

                let result =
                    self.app_state.db.get_relayers_for_chain(&chain_id, paging_context).await?;

                Ok(result)
            }
            None => {
                let result = self.app_state.db.get_relayers(paging_context).await?;

                Ok(result)
            }
        }
    }

    /// Convenience helper returning just the relayer's address.
    pub async fn get_relayer_address(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<Option<EvmAddress>, HttpError> {
        let relayer = get_relayer(&self.app_state.db, &self.app_state.cache, relayer_id).await?;

        Ok(relayer.map(|relayer| relayer.address))
    }
}
