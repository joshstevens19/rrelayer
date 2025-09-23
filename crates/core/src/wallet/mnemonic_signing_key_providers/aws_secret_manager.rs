use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;

use crate::wallet::WalletError;
use crate::yaml::AwsSecretManager;

pub async fn get_aws_secret(config: &AwsSecretManager) -> Result<String, WalletError> {
    let aws_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(config.region.clone()))
        .load()
        .await;

    let client = Client::new(&aws_config);

    let resp =
        client.get_secret_value().secret_id(config.id.clone()).send().await.map_err(|e| {
            WalletError::ApiError { message: format!("Failed to get AWS secret: {}", e) }
        })?;

    let secret_string = resp
        .secret_string()
        .ok_or(WalletError::ApiError { message: "failed to get secret string".to_string() })?;

    let secret_json: serde_json::Value = serde_json::from_str(secret_string)?;

    let key_value = secret_json.get(&config.key).and_then(|v| v.as_str()).ok_or(
        WalletError::ConfigurationError {
            message: format!("Key '{}' not found in secret or is not a string", config.key),
        },
    )?;

    Ok(key_value.to_string())
}
