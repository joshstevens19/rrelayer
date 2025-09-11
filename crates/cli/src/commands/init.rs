use std::{fs, path::Path};

use dialoguer::{Confirm, Input};
use rrelayer_core::{
    ApiConfig, NetworkSetupConfig, RawSigningKey, SetupConfig, SigningKey, WriteFileError,
    generate_docker_file, generate_seed_phrase, write_file,
};
use serde_yaml;

use crate::project_location::ProjectLocation;
use crate::{commands::error::InitError, console::print_error_message};

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

    let project_path = path.join(&project_name);

    fs::create_dir(&project_path)?;

    let mut project_location = ProjectLocation::new(project_path.clone());
    project_location.override_project_name(&project_name);

    let yaml_content: SetupConfig = SetupConfig {
        name: project_name.clone(),
        description: if !project_description.is_empty() { Some(project_description) } else { None },
        signing_key: Some(SigningKey::from_raw(RawSigningKey {
            mnemonic: "${RAW_DANGEROUS_MNEMONIC}".to_string(),
        })),
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
        safe_proxy: None,
        user_rate_limits: None,
    };
    fs::write(project_path.join("rrelayer.yaml"), serde_yaml::to_string(&yaml_content)?)?;

    let phrase = generate_seed_phrase()?;

    if docker_support {
        let env = format!(
            "RAW_DANGEROUS_MNEMONIC=\"{}\"\nDATABASE_URL=postgresql://postgres:rrelayer@localhost:5441/postgres\nPOSTGRES_PASSWORD=rrelayer\nRRELAYER_AUTH_USERNAME=your_username\nRRELAYER_AUTH_PASSWORD=your_password\n",
            phrase
        );

        write_docker_compose(&project_path).map_err(|e| {
            print_error_message(&format!("Failed to write docker compose file: {}", e));
            InitError::ConfigWrite(e)
        })?;

        write_file(&project_path.join(".env"), &env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            InitError::ConfigWrite(e)
        })?;
    } else {
        let env = format!(
            "RAW_DANGEROUS_MNEMONIC=\"{}\"\nDATABASE_URL=postgresql://[user[:password]@][host][:port][/dbname]\nRRELAYER_AUTH_USERNAME=your_username\nRRELAYER_AUTH_PASSWORD=your_password\n",
            phrase
        );

        write_file(&project_path.join(".env"), &env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            InitError::ConfigWrite(e)
        })?;
    }

    write_gitignore(&project_path).map_err(InitError::ConfigWrite)?;

    println!(
        "\nProject '{}' initialized successfully! note we advise to not use the RAW_DANGEROUS_MNEMONIC in production and use one of the secure key management signing keys. Alongside replace RRELAYER_AUTH_USERNAME and RRELAYER_AUTH_PASSWORD in the .env",
        project_name
    );

    Ok(())
}
