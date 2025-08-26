use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::{config::Credentials, Client};

use crate::wallet::WalletError;
use crate::yaml::AwsSigningKey;

/// Retrieves a secret value from AWS Secrets Manager.
///
/// This function authenticates with AWS using the provided credentials and retrieves
/// a secret from AWS Secrets Manager. The secret is expected to be stored as JSON,
/// and this function extracts a specific key from that JSON.
///
/// # Arguments
/// * `config` - AWS configuration containing credentials, region, secret name, and key
///
/// # Returns
/// * `Ok(String)` - The secret value for the specified key
/// * `Err(WalletError)` - If AWS API call fails, secret is not found, or key extraction fails
///
/// # Errors
/// - `WalletError::ApiError` - If AWS API call fails or secret string is missing
/// - `WalletError::JsonError` - If secret JSON parsing fails
/// - `WalletError::ConfigurationError` - If the specified key is not found in the secret
pub async fn get_aws_secret(config: &AwsSigningKey) -> Result<String, WalletError> {
    let credentials = Credentials::new(
        config.access_key_id.clone(),
        config.secret_access_key.clone(),
        config.session_token.clone(),
        None,
        "rrelayer-server",
    );

    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new(config.region.clone()))
        .load()
        .await;

    let client = Client::new(&shared_config);

    let resp =
        client.get_secret_value().secret_id(config.secret_name.clone()).send().await.map_err(
            |e| WalletError::ApiError { message: format!("Failed to get AWS secret: {}", e) },
        )?;

    let secret_string = resp
        .secret_string()
        .ok_or(WalletError::ApiError { message: "failed to get secret string".to_string() })?;

    let secret_json: serde_json::Value = serde_json::from_str(secret_string)?;

    let key_value = secret_json.get(&config.secret_key).and_then(|v| v.as_str()).ok_or(
        WalletError::ConfigurationError {
            message: format!("Key '{}' not found in secret or is not a string", config.secret_key),
        },
    )?;

    Ok(key_value.to_string())
}
