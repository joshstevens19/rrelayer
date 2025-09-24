use crate::wallet::WalletError;
use crate::SigningKey;
use std::path::PathBuf;

mod aws_secret_manager;
use aws_secret_manager::get_aws_secret;
mod gcp_secret_manager;
use gcp_secret_manager::get_gcp_secret;

pub async fn get_mnemonic_from_signing_key(
    project_path: &PathBuf,
    signing_key: &SigningKey,
) -> Result<String, WalletError> {
    println!("mnemonic LOAD BABY");
    if let Some(raw) = &signing_key.raw {
        return Ok(raw.mnemonic.clone());
    }

    if let Some(aws_secret_manager) = &signing_key.aws_secret_manager {
        let result = get_aws_secret(aws_secret_manager).await?;
        return Ok(result);
    }

    if let Some(gcp_secret_manager) = &signing_key.gcp_secret_manager {
        let result = get_gcp_secret(project_path, gcp_secret_manager).await?;
        return Ok(result);
    }

    Err(WalletError::NoSigningKey)
}
