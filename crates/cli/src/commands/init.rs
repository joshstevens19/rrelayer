use std::{fs, path::Path};

use dialoguer::{Confirm, Input, Password};
use rrelayerr_core::{
    GasProviders, KeystoreSigningKey, NetworkSetupConfig, SetupConfig, SigningKey, WriteFileError,
    gas::fee_estimator::tenderly::TenderlyGasProviderSetupConfig, generate_docker_file, write_file,
};
use serde_yaml;

use crate::{
    commands::keystore::{ProjectLocation, create_from_mnemonic},
    console::print_error_message,
};

fn write_docker_compose(path: &Path) -> Result<(), WriteFileError> {
    write_file(&path.join("docker-compose.yml"), generate_docker_file())
}

fn write_gitignore(path: &Path) -> Result<(), WriteFileError> {
    write_file(
        &path.join(".gitignore"),
        r#".env
    "#,
    )
}

pub async fn handle_init(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let project_name: String = Input::new().with_prompt("Enter project name").interact_text()?;

    let project_description: String = Input::new()
        .with_prompt("Enter project description (optional)")
        .allow_empty(true)
        .interact_text()?;

    let docker_support = Confirm::new()
        .with_prompt("Do you want Docker support out of the box (will make it easy to run)?")
        .default(true)
        .interact()?;

    let mnemonic_password = Password::new()
        .with_prompt("Enter password to encrypt keystore file for the relayers signing key")
        .with_confirmation("Confirm password", "Passwords don't match")
        .interact()?;

    let project_path = path.join(&project_name);

    fs::create_dir(&project_path)?;

    let mut project_location = ProjectLocation::new(project_path.clone());
    project_location.override_project_name(&project_name);

    let account_name = "rrelayerr_signing_key";
    let created_path = create_from_mnemonic(
        &None,
        true,
        &account_name,
        project_location,
        Some(mnemonic_password),
    )?;

    let relative_path = if created_path.starts_with(&project_path) {
        let path_diff = created_path
            .strip_prefix(&project_path)
            .map(|p| format!("./{}", p.display()))
            .unwrap_or_else(|_| format!("./keystores/{}", account_name));
        path_diff
    } else {
        format!("./keystores/{}", account_name)
    };

    let yaml_content: SetupConfig = SetupConfig {
        name: project_name.clone(),
        description: if !project_description.is_empty() { Some(project_description) } else { None },
        signing_key: Some(SigningKey::from_keystore(KeystoreSigningKey {
            path: relative_path,
            account_name: account_name.to_string(),
        })),
        admins: vec![],
        networks: vec![NetworkSetupConfig {
            name: "sepolia_ethereum".to_string(),
            signing_key: None,
            provider_urls: vec!["https://sepolia.gateway.tenderly.co".to_string()],
            block_explorer_url: Some("https://sepolia.etherscan.io".to_string()),
            gas_provider: None,
        }],
        gas_providers: Some(GasProviders {
            infura: None,
            tenderly: Some(TenderlyGasProviderSetupConfig { enabled: true, api_key: None }),
            custom: None,
        }),
        allowed_origins: None,
    };
    fs::write(project_path.join("rrelayerr.yaml"), serde_yaml::to_string(&yaml_content)?)?;

    if docker_support {
        let env = r#"DATABASE_URL=postgresql://postgres:rrelayerr@localhost:5441/postgres
POSTGRES_PASSWORD=rrelayerr"#;

        write_docker_compose(&project_path).map_err(|e| {
            print_error_message(&format!("Failed to write docker compose file: {}", e));
            e
        })?;

        write_file(&project_path.join(".env"), &env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            e
        })?;
    } else {
        let env = r#"DATABASE_URL=postgresql://[user[:password]@][host][:port][/dbname]"#;

        write_file(&project_path.join(".env"), env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            e
        })?;
    }

    write_gitignore(&project_path)?;

    println!("\nProject '{}' initialized successfully!", project_name);
    Ok(())
}
