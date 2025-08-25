use std::{env, path::PathBuf, process::Command, thread, time::Duration};

use rrelayer_core::{PostgresClient, rrelayer_error, rrelayer_info, start};

use crate::{commands::error::ProjectStartupError, console::print_error_message};

pub async fn handle_start(project_path: &PathBuf) -> Result<(), ProjectStartupError> {
    rrelayer_info!("Loading from path {:?}", project_path);
    let rrelayer_yaml_path = project_path.join("rrelayer.yaml");
    if !rrelayer_yaml_path.exists() {
        return Err(ProjectStartupError::NotInitialized(
            "Not in a relayer project directory. Please run this command from your project root."
                .to_string(),
        ));
    }

    rrelayer_info!("Starting relayer...");

    let docker_compose_path = project_path.join("docker-compose.yml");
    if !docker_compose_path.exists() {
        return Err(ProjectStartupError::DockerComposeNotFound);
    }

    match start_docker_compose(&project_path) {
        Ok(_) => {
            rrelayer_info!("Docker postgres containers started up successfully");
        }
        Err(e) => {
            return Err(e);
        }
    }

    // check connection of the database
    let _ = PostgresClient::new().await?;

    start(project_path).await?;

    Ok(())
}

fn check_docker_compose_status(
    project_path: &PathBuf,
    max_retries: u32,
) -> Result<(), ProjectStartupError> {
    let mut retries = 0;

    while retries < max_retries {
        let ps_status = Command::new("docker")
            .args(["compose", "ps"])
            .current_dir(project_path)
            .output()
            .map_err(|e| {
                let error = format!("Failed to check docker compose status: {}", e);
                print_error_message(&error);
                ProjectStartupError::DockerFailed(error)
            })?;

        if ps_status.status.success() {
            let output = String::from_utf8_lossy(&ps_status.stdout);
            if !output.contains("Exit") && output.contains("Up") {
                rrelayer_info!("All containers are up and running.");

                return if let Ok(_) = env::var("DATABASE_URL") {
                    Ok(())
                } else {
                    let error = "DATABASE_URL not set.".to_string();
                    rrelayer_error!(error);
                    Err(ProjectStartupError::MissingEnvironmentVariable("DATABASE_URL".to_string()))
                };
            }
        } else {
            let error = format!("docker compose ps exited with status: {}", ps_status.status);
            rrelayer_error!(error);
        }

        retries += 1;
        thread::sleep(Duration::from_millis(200));
        rrelayer_info!("Waiting for docker compose containers to start...");
    }

    Err("Docker containers did not start successfully within the given retries.".into())
}

fn start_docker_compose(project_path: &PathBuf) -> Result<(), ProjectStartupError> {
    if !project_path.exists() {
        return Err(ProjectStartupError::InvalidConfig(format!(
            "Project path does not exist: {:?}",
            project_path
        )));
    }

    let status = Command::new("docker")
        .args(["compose", "up", "-d"])
        .current_dir(project_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| {
            let error = format!("Docker command could not be executed make sure docker is running on the machine: {}", e);
            print_error_message(&error);
            ProjectStartupError::DockerFailed(error)
        })?;

    if !status.success() {
        let error = "Docker compose could not startup the postgres container, please make sure docker is running on the machine".to_string();
        rrelayer_error!(error);
        return Err(ProjectStartupError::DockerFailed(error));
    }

    rrelayer_info!("Docker starting up the postgres container..");

    check_docker_compose_status(project_path, 200)
}
