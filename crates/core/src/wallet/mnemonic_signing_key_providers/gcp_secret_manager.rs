use crate::wallet::WalletError;
use crate::yaml::GcpSecretManagerProviderConfig;
use google_secretmanager1::{hyper_rustls, hyper_util, yup_oauth2, SecretManager};
use std::path::Path;

pub async fn get_gcp_secret(
    project_path: &Path,
    config: &GcpSecretManagerProviderConfig,
) -> Result<String, WalletError> {
    let key_path = project_path.join(&config.service_account_key_path);

    let service_account_key =
        yup_oauth2::read_service_account_key(&key_path).await.map_err(|e| {
            WalletError::ConfigurationError {
                message: format!("Failed to read service account key: {}", e),
            }
        })?;

    let project_id =
        service_account_key.project_id.clone().ok_or(WalletError::ConfigurationError {
            message: "No project_id found in service account key".to_string(),
        })?;

    let auth = yup_oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .map_err(|e| WalletError::AuthenticationError {
            message: format!("Failed to create authenticator: {}", e),
        })?;

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .map_err(|e| WalletError::ConfigurationError {
            message: format!("Failed to create HTTPS connector: {}", e),
        })?
        .https_or_http()
        .enable_http1()
        .build();

    let hyper_client =
        hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .build(https);

    let client = SecretManager::new(hyper_client, auth);

    let version = config.version.as_deref().unwrap_or("latest");
    let secret_path = format!("projects/{}/secrets/{}/versions/{}", project_id, config.id, version);

    let result =
        client.projects().secrets_versions_access(&secret_path).doit().await.map_err(|e| {
            WalletError::ApiError { message: format!("Failed to access secret: {}", e) }
        })?;

    let secret_data = result
        .1
        .payload
        .and_then(|payload| payload.data)
        .ok_or(WalletError::ApiError { message: "No secret data found".to_string() })?;

    let secret_string = String::from_utf8(secret_data)?;

    let secret_key = config.key.clone();
    let secret_json: serde_json::Value = serde_json::from_str(&secret_string)?;

    let key_value = secret_json.get(&secret_key).and_then(|v| v.as_str()).ok_or(
        WalletError::ConfigurationError {
            message: format!("Key '{}' not found in secret or is not a string", secret_key),
        },
    )?;

    Ok(key_value.to_string())
}
