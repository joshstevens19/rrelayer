use std::{fs, path::Path};

use dialoguer::{Confirm, Input};
use rand::{Rng, distributions::Alphanumeric};
use rrelayer_core::{
    AdminIdentifier, ApiConfig, KeystoreSigningKey, NetworkSetupConfig, SetupConfig, SigningKey,
    WriteFileError, generate_docker_file, keystore::recover_wallet_from_keystore, rrelayer_info,
    write_file,
};
use serde_yaml;

use crate::{
    commands::error::InitError,
    commands::keystore::{ProjectLocation, create_from_mnemonic, create_from_private_key},
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

pub async fn handle_init(path: &Path) -> Result<(), InitError> {
    let project_name: String = Input::new().with_prompt("Enter project name").interact_text()?;

    let project_description: String = Input::new()
        .with_prompt("Enter project description (optional)")
        .allow_empty(true)
        .interact_text()?;

    let docker_support = Confirm::new()
        .with_prompt("Do you want Docker support out of the box (will make it easy to run)?")
        .default(true)
        .interact()?;

    let mnemonic_password =
        rand::thread_rng().sample_iter(&Alphanumeric).take(24).map(char::from).collect::<String>();

    let project_path = path.join(&project_name);

    fs::create_dir(&project_path)?;

    let mut project_location = ProjectLocation::new(project_path.clone());
    project_location.override_project_name(&project_name);

    let mnemonic_name = "rrelayer_signing_key";
    let created_path = create_from_mnemonic(
        &None,
        true,
        &mnemonic_name,
        project_location.clone(),
        Some(mnemonic_password.clone()),
    )?;

    let relative_path = if created_path.starts_with(&project_path) {
        let path_diff = created_path
            .strip_prefix(&project_path)
            .map(|p| format!("./{}", p.display()))
            .unwrap_or_else(|_| format!("./keystores/{}", mnemonic_name));
        path_diff
    } else {
        format!("./keystores/{}", mnemonic_name)
    };

    let account_password =
        rand::thread_rng().sample_iter(&Alphanumeric).take(24).map(char::from).collect::<String>();

    let account_name = "account1";

    let account_path = create_from_private_key(
        &None,
        true,
        account_name,
        project_location,
        Some(account_password.clone()),
    )?;

    // make sure it works properly
    recover_wallet_from_keystore(&account_path, &account_password)
        .map_err(|e| InitError::Wallet(e))?;

    let yaml_content: SetupConfig = SetupConfig {
        name: project_name.clone(),
        description: if !project_description.is_empty() { Some(project_description) } else { None },
        signing_key: Some(SigningKey::from_keystore(KeystoreSigningKey {
            path: relative_path,
            name: mnemonic_name.to_string(),
            dangerous_define_raw_password: None,
        })),
        admins: vec![AdminIdentifier::Name(account_name.to_string())],
        networks: vec![NetworkSetupConfig {
            name: "sepolia_ethereum".to_string(),
            signing_key: None,
            provider_urls: vec!["https://sepolia.gateway.tenderly.co".to_string()],
            block_explorer_url: Some("https://sepolia.etherscan.io".to_string()),
            gas_provider: None,
            automatic_top_up: None,
            confirmations: None,
        }],
        gas_providers: None,
        api_config: ApiConfig { port: 8000, allowed_origins: None },
        webhooks: None,
    };
    fs::write(project_path.join("rrelayer.yaml"), serde_yaml::to_string(&yaml_content)?)?;

    if docker_support {
        let env = r#"DATABASE_URL=postgresql://postgres:rrelayer@localhost:5441/postgres
POSTGRES_PASSWORD=rrelayer"#;

        write_docker_compose(&project_path).map_err(|e| {
            print_error_message(&format!("Failed to write docker compose file: {}", e));
            InitError::ConfigWrite(e)
        })?;

        write_file(&project_path.join(".env"), &env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            InitError::ConfigWrite(e)
        })?;
    } else {
        let env = r#"DATABASE_URL=postgresql://[user[:password]@][host][:port][/dbname]"#;

        write_file(&project_path.join(".env"), env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            InitError::ConfigWrite(e)
        })?;
    }

    write_gitignore(&project_path).map_err(InitError::ConfigWrite)?;

    rrelayer_info!("\nProject '{}' initialized successfully!", project_name);
    rrelayer_info!(
        "Secured with the signing key with the password: {} please write it down it has auto logged you in on this system",
        mnemonic_password
    );
    rrelayer_info!(
        "Created you `account1` secured with password: {} please write it down it has auto logged you in on this system",
        account_password
    );
    Ok(())
}
