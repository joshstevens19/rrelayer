use std::{fs, path::Path};

use dialoguer::{Confirm, Input};

pub fn handle_init() -> Result<(), Box<dyn std::error::Error>> {
    // Project name input (required)
    let project_name: String =
        Input::new().with_prompt("Enter relayer project name").interact_text()?;

    // Project description input (optional)
    let project_description: String = Input::new()
        .with_prompt("Enter project description (optional)")
        .allow_empty(true)
        .interact_text()?;

    // Docker support confirmation
    let docker_support = Confirm::new()
        .with_prompt("Do you want Docker support out of the box?")
        .default(true)
        .interact()?;

    // Create project directory
    fs::create_dir(&project_name)?;

    // Create rrelayer.yaml
    let yaml_content = format!(
        "project_name: {}\ndescription: {}\ndocker_support: {}\n",
        project_name, project_description, docker_support
    );
    fs::write(Path::new(&project_name).join("rrelayer.yaml"), yaml_content)?;

    // Create .env file
    let env_content = format!("PROJECT_NAME={}\nDOCKER_ENABLED={}\n", project_name, docker_support);
    fs::write(Path::new(&project_name).join("../../../../.env"), env_content)?;

    // Create Dockerfile if docker support is enabled
    if docker_support {
        let dockerfile_content = r#"FROM rust:latest
WORKDIR /app
COPY . .
RUN cargo build --release
CMD ["./target/release/relayer"]"#;
        fs::write(Path::new(&project_name).join("Dockerfile"), dockerfile_content)?;
    }

    println!("\nProject '{}' initialized successfully!", project_name);
    Ok(())
}
