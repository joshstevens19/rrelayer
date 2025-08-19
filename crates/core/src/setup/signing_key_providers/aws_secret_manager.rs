use aws_config::{BehaviorVersion, Region};
use aws_sdk_secretsmanager::{config::Credentials, Client};

use crate::setup::yaml::AwsSigningKey;

pub async fn get_aws_secret(
    config: &AwsSigningKey,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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

    let resp = client.get_secret_value().secret_id(config.secret_name.clone()).send().await?;

    let secret_string = resp.secret_string().ok_or("failed to get secret string")?;

    let secret_json: serde_json::Value = serde_json::from_str(secret_string)
        .map_err(|e| format!("Failed to parse secret as JSON: {}", e))?;

    let key_value = secret_json
        .get(&config.secret_key)
        .and_then(|v| v.as_str())
        .ok_or(format!("Key '{}' not found in secret or is not a string", config.secret_key))?;

    Ok(key_value.to_string())
}
