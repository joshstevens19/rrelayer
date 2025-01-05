use self::aws_secret_manager::get_aws_secret;
use super::yaml::SigningKey;

mod aws_secret_manager;

pub async fn get_mnemonic_from_signing_key(
    signing_key: &SigningKey,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(raw) = &signing_key.raw {
        return Ok(raw.mnemonic.clone());
    }

    if let Some(aws_secret_manager) = &signing_key.aws_secret_manager {
        let result = get_aws_secret(aws_secret_manager).await?;
        return Ok(result);
    }

    Err("No signing key found".into())
}
