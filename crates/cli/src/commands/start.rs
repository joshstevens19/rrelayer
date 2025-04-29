use std::{env, path::PathBuf, process::Command, thread, time::Duration};

use clap::Args;
use rrelayerr_core::{PostgresClient, rrelayerr_error, rrelayerr_info, start};

use crate::console::{print_error_message, print_success_message};

#[derive(Args)]
pub struct StartArgs {
    /// optional - To start the project from, default will be where the command is run.
    #[clap(long, short)]
    pub path: Option<String>,
}

pub async fn handle_start(
    _args: &StartArgs,
    project_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    rrelayerr_info!("Loading from path {:?}", project_path);
    let rrelayerr_yaml_path = project_path.join("rrelayerr.yaml");
    if !rrelayerr_yaml_path.exists() {
        return Err(
            "Not in a relayer project directory. Please run this command from your project root."
                .into(),
        );
    }

    rrelayerr_info!("Starting relayer...");

    let docker_compose_path = project_path.join("docker-compose.yml");
    if !docker_compose_path.exists() {
        return Err("The DATABASE_URL mapped is not running please make sure it is correct".into());
    }

    match start_docker_compose(&project_path) {
        Ok(_) => {
            rrelayerr_info!("Docker postgres containers started up successfully");
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    // check connection of the database
    let _ = PostgresClient::new().await?;

    start(project_path).await?;

    Ok(())
}

fn check_postgres_connection(conn_str: &str, max_retries: u32) -> Result<(), String> {
    let mut retries = 0;

    while retries < max_retries {
        let status = Command::new("pg_isready").args(["-d", conn_str]).output().map_err(|e| {
            let error = format!("Failed to check Postgres status: {}", e);
            rrelayerr_error!(error);
            error
        })?;

        if status.status.success() {
            return Ok(());
        }

        retries += 1;
        thread::sleep(Duration::from_millis(500));
        rrelayerr_info!(
            "Waiting for Postgres to become available this may take a few attempts... attempt: {}",
            retries
        );
    }

    Err("Postgres did not become available within the given retries.".into())
}

fn check_docker_compose_status(project_path: &PathBuf, max_retries: u32) -> Result<(), String> {
    let mut retries = 0;

    while retries < max_retries {
        let ps_status = Command::new("docker")
            .args(["compose", "ps"])
            .current_dir(project_path)
            .output()
            .map_err(|e| {
                let error = format!("Failed to check docker compose status: {}", e);
                print_error_message(&error);
                error
            })?;

        if ps_status.status.success() {
            let output = String::from_utf8_lossy(&ps_status.stdout);
            if !output.contains("Exit") && output.contains("Up") {
                rrelayerr_info!("All containers are up and running.");

                return if let Ok(conn_str) = env::var("DATABASE_URL") {
                    check_postgres_connection(&conn_str, max_retries).map_err(|e| {
                        let error = format!("Failed to connect to PostgresSQL: {}", e);
                        rrelayerr_error!(error);
                        error
                    })
                } else {
                    let error = "DATABASE_URL not set.".to_string();
                    rrelayerr_error!(error);
                    Err(error)
                };
            }
        } else {
            let error = format!("docker compose ps exited with status: {}", ps_status.status);
            rrelayerr_error!(error);
        }

        retries += 1;
        thread::sleep(Duration::from_millis(200));
        rrelayerr_info!("Waiting for docker compose containers to start...");
    }

    Err("Docker containers did not start successfully within the given retries.".into())
}

fn start_docker_compose(project_path: &PathBuf) -> Result<(), String> {
    if !project_path.exists() {
        return Err(format!("Project path does not exist: {:?}", project_path));
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
            error
        })?;

    if !status.success() {
        let error = "Docker compose could not startup the postgres container, please make sure docker is running on the machine".to_string();
        rrelayerr_error!(error);
        return Err(error);
    }

    rrelayerr_info!("Docker starting up the postgres container..");

    check_docker_compose_status(project_path, 200)
}
