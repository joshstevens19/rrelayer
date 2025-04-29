use crate::{setup::signing_key_providers::aws_secret_manager::get_aws_secret, SigningKey};

pub mod aws_secret_manager;
pub mod keystore;

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
