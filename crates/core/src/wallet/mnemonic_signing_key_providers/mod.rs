use crate::wallet::WalletError;
use crate::SigningProvider;
use std::path::Path;

#[cfg(feature = "aws")]
mod aws_secret_manager;
#[cfg(feature = "aws")]
use aws_secret_manager::get_aws_secret;
#[cfg(feature = "gcp")]
mod gcp_secret_manager;
#[cfg(feature = "gcp")]
use gcp_secret_manager::get_gcp_secret;

#[allow(unused_variables)]
pub async fn get_mnemonic_from_signing_key(
    project_path: &Path,
    signing_key: &SigningProvider,
) -> Result<String, WalletError> {
    if let Some(raw) = &signing_key.raw {
        return Ok(raw.mnemonic.clone());
    }

    #[cfg(feature = "aws")]
    if let Some(aws_secret_manager) = &signing_key.aws_secret_manager {
        let result = get_aws_secret(aws_secret_manager).await?;
        return Ok(result);
    }

    #[cfg(feature = "gcp")]
    if let Some(gcp_secret_manager) = &signing_key.gcp_secret_manager {
        let result = get_gcp_secret(project_path, gcp_secret_manager).await?;
        return Ok(result);
    }

    Err(WalletError::NoSigningKey)
}
