use std::path::PathBuf;

use crate::{
    keystore::{recover_mnemonic_from_keystore, KeyStorePasswordManager},
    setup::signing_key_providers::aws_secret_manager::get_aws_secret,
    SigningKey,
};

pub mod aws_secret_manager;
pub mod keystore;

pub async fn get_mnemonic_from_signing_key(
    project_path: &PathBuf,
    project_name: &str,
    signing_key: &SigningKey,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(raw) = &signing_key.raw {
        return Ok(raw.mnemonic.clone());
    }

    if let Some(aws_secret_manager) = &signing_key.aws_secret_manager {
        let result = get_aws_secret(aws_secret_manager).await?;
        return Ok(result);
    }

    if let Some(keystore) = &signing_key.keystore {
        let password = keystore.dangerous_define_raw_password
            .clone()
            .unwrap_or_else(|| {
                KeyStorePasswordManager::new(project_name)
                    .load(&keystore.name)
                    .expect("Server is not authenticated to use the keystores rrelayerr_signing_key please login on the server")
            });
        let keystore_path = project_path.join(&keystore.path);
        let result = recover_mnemonic_from_keystore(&keystore_path, &password)?;
        return Ok(result);
    }

    Err("No signing key found".into())
}
