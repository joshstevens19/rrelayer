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
        "rrelayerr-server",
    );

    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new(config.region.clone()))
        .load()
        .await;

    let client = Client::new(&shared_config);

    let resp = client.get_secret_value().secret_id(config.secret_name.clone()).send().await?;

    match resp.secret_string() {
        Some(secret_string) => Ok(secret_string.to_string()),
        None => Err("failed to get secret string".into()),
    }
}
