use std::{fs, path::Path};

use dialoguer::{Confirm, Input};
use rrelayerr::{
    NetworkSetupConfig, SetupConfig, SigningKey, WriteFileError, generate_docker_file,
    generate_seed_phrase, write_file,
};
use serde_yaml;

use crate::console::print_error_message;

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
        .with_prompt("Do you want Docker support out of the box?")
        .default(true)
        .interact()?;

    let project_path = path.join(&project_name);

    fs::create_dir(&project_path)?;

    let yaml_content: SetupConfig = SetupConfig {
        name: project_name.clone(),
        description: if !project_description.is_empty() { Some(project_description) } else { None },
        signing_key: Some(SigningKey::default()),
        admins: vec![],
        networks: vec![NetworkSetupConfig {
            name: "sepolia_ethereum".to_string(),
            signing_key: None,
            provider_urls: vec!["https://sepolia.gateway.tenderly.co".to_string()],
            block_explorer_url: Some("https://sepolia.etherscan.io".to_string()),
            gas_provider: None,
        }],
        gas_providers: None,
        allowed_origins: None,
    };
    fs::write(project_path.join("rrelayerr.yaml"), serde_yaml::to_string(&yaml_content)?)?;

    let new_mnemonic = Confirm::new()
        .with_prompt("Do you want rrelayerr to generate a new mnemonic for you?")
        .default(true)
        .interact()?;

    let mut env = if new_mnemonic {
        let seed_phrase = generate_seed_phrase()?;
        format!("MNEMONIC=\"{}\"\"\n", seed_phrase)
    } else {
        "MNEMONIC=INSERT_YOUR_MNEMONIC_HERE\n".to_string()
    };

    if docker_support {
        env += r#"DATABASE_URL=postgresql://postgres:rrelayerr@localhost:5441/postgres
POSTGRES_PASSWORD=rrelayerr"#;

        write_docker_compose(&project_path).map_err(|e| {
            print_error_message(&format!("Failed to write docker compose file: {}", e));
            e
        })?;
    } else {
        let env = r#"DATABASE_URL=postgresql://[user[:password]@][host][:port][/dbname]"#;

        write_file(&project_path.join(".env"), env).map_err(|e| {
            print_error_message(&format!("Failed to write .env file: {}", e));
            e
        })?;
    }

    write_file(&project_path.join(".env"), &env).map_err(|e| {
        print_error_message(&format!("Failed to write .env file: {}", e));
        e
    })?;

    write_gitignore(&project_path)?;

    println!("\nProject '{}' initialized successfully!", project_name);
    Ok(())
}
