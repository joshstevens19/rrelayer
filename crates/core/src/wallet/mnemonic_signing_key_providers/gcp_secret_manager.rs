use crate::wallet::WalletError;
use crate::yaml::GcpSigningKey;
use google_secretmanager1::{hyper, hyper_rustls, oauth2, SecretManager};
use std::path::PathBuf;

/// Retrieves a secret value from Google Cloud Secret Manager.
///
/// This function authenticates with Google Cloud using a service account key file
/// and retrieves a secret from Google Cloud Secret Manager. The secret is expected
/// to be stored as JSON, and this function extracts a specific key from that JSON.
///
/// # Arguments
/// * `project_path` - Path to the project directory for resolving the service account key file
/// * `config` - GCP configuration containing service account key path, secret name, and key
///
/// # Returns
/// * `Ok(String)` - The secret value for the specified key
/// * `Err(WalletError)` - If authentication fails, API call fails, or key extraction fails
///
/// # Errors
/// - `WalletError::ConfigurationError` - If service account key file cannot be read or HTTPS connector fails
/// - `WalletError::AuthenticationError` - If service account authentication fails
/// - `WalletError::ApiError` - If GCP API call fails or secret data is missing
/// - `WalletError::JsonError` - If secret JSON parsing fails
/// - `WalletError::StringEncodingError` - If secret data is not valid UTF-8
pub async fn get_gcp_secret(
    project_path: &PathBuf,
    config: &GcpSigningKey,
) -> Result<String, WalletError> {
    let key_path = project_path.join(&config.service_account_key_path);

    let service_account_key = oauth2::read_service_account_key(&key_path).await.map_err(|e| {
        WalletError::ConfigurationError {
            message: format!("Failed to read service account key: {}", e),
        }
    })?;

    let project_id =
        service_account_key.project_id.clone().ok_or(WalletError::ConfigurationError {
            message: "No project_id found in service account key".to_string(),
        })?;

    let auth = oauth2::ServiceAccountAuthenticator::builder(service_account_key)
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

    let client = SecretManager::new(hyper::Client::builder().build(https), auth);

    let version = config.version.as_deref().unwrap_or("latest");
    let secret_path =
        format!("projects/{}/secrets/{}/versions/{}", project_id, config.secret_name, version);

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

    let secret_key = config.secret_key.clone();
    let secret_json: serde_json::Value = serde_json::from_str(&secret_string)?;

    let key_value = secret_json.get(&secret_key).and_then(|v| v.as_str()).ok_or(
        WalletError::ConfigurationError {
            message: format!("Key '{}' not found in secret or is not a string", secret_key),
        },
    )?;

    Ok(key_value.to_string())
}
