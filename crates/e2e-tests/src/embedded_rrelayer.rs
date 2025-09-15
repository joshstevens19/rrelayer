use anyhow::Result;
use rrelayer_core::{start, StartError};
use std::{path::PathBuf, process::Command, thread, time::Duration};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct EmbeddedRRelayerServer {
    project_path: PathBuf,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl EmbeddedRRelayerServer {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path, server_handle: None }
    }

    /// Start the RRelayer server in a background task
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting embedded RRelayer server...");

        // Start Docker Compose services (PostgreSQL)
        self.start_docker_compose()?;

        // Drop all database schemas for a fresh start
        self.reset_database().await?;

        let project_path = self.project_path.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_server(&project_path).await {
                error!("❌ RRelayer server failed: {}", e);
            }
        });

        self.server_handle = Some(handle);

        // Wait for the server to be ready
        self.wait_for_ready().await?;

        // Give the server a moment to fully initialize after health check
        sleep(Duration::from_millis(500)).await;

        info!("✅ Embedded RRelayer server is ready!");
        Ok(())
    }

    /// Wait for the server to be ready by checking the health endpoint
    async fn wait_for_ready(&self) -> Result<()> {
        let mut retries = 30;

        while retries > 0 {
            match reqwest::get("http://localhost:3000/health").await {
                Ok(response) if response.status().is_success() => {
                    info!("✅ RRelayer health check passed");
                    return Ok(());
                }
                Ok(response) => {
                    warn!("⚠️  Health check returned {}, retrying...", response.status());
                }
                Err(e) => {
                    warn!("⚠️  Health check failed: {}, retrying...", e);
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
            retries -= 1;
        }

        Err(anyhow::anyhow!("RRelayer server did not become ready after 60 seconds"))
    }

    /// The actual server startup logic - uses rrelayer_core::start
    async fn run_server(project_path: &PathBuf) -> Result<(), StartError> {
        info!("Starting up the embedded RRelayer server");
        start(project_path).await
    }

    /// Reset database by dropping all schemas for a fresh start
    async fn reset_database(&self) -> Result<()> {
        info!("🗑️  Resetting database schemas for fresh E2E test run...");

        // Database connection settings for E2E tests
        let connection_string =
            std::env::var("DATABASE_URL").expect("DATABASE_URL needs to be set");

        let schemas_to_drop = vec!["public", "rate_limit", "signing", "network", "relayer"];

        // Try to connect and drop schemas
        match tokio_postgres::connect(&connection_string, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                // Spawn the connection task
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        error!("Database connection error: {}", e);
                    }
                });

                for schema in &schemas_to_drop {
                    let drop_query = format!("DROP SCHEMA IF EXISTS {} CASCADE", schema);
                    match client.execute(&drop_query, &[]).await {
                        Ok(_) => info!("✅ Dropped schema: {}", schema),
                        Err(e) => warn!("⚠️  Failed to drop schema {}: {}", schema, e),
                    }
                }

                // Recreate the public schema
                match client.execute("CREATE SCHEMA public", &[]).await {
                    Ok(_) => info!("✅ Recreated public schema"),
                    Err(e) => warn!("⚠️  Failed to recreate public schema: {}", e),
                }

                info!("✅ Database reset complete");
            }
            Err(e) => {
                warn!("⚠️  Could not connect to database for reset: {}. Continuing anyway...", e);
                warn!("   Make sure PostgreSQL is running with the correct credentials");
            }
        }

        Ok(())
    }

    /// Start Docker Compose services for E2E tests
    fn start_docker_compose(&self) -> Result<()> {
        info!("🐳 Starting Docker Compose services...");

        if !self.project_path.exists() {
            return Err(anyhow::anyhow!("Project path does not exist: {:?}", self.project_path));
        }

        let status = Command::new("docker")
            .args(["compose", "up", "-d"])
            .current_dir(&self.project_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| {
                let error = format!(
                    "Docker command could not be executed. Make sure Docker is running: {}",
                    e
                );
                error!("{}", error);
                anyhow::anyhow!(error)
            })?;

        if !status.success() {
            let error = "Docker Compose could not start PostgreSQL container. Please make sure Docker is running.";
            error!("{}", error);
            return Err(anyhow::anyhow!(error));
        }

        info!("🐳 Docker starting up PostgreSQL container...");

        self.check_docker_compose_status(200)
    }

    /// Check Docker Compose status with retries
    fn check_docker_compose_status(&self, max_retries: u32) -> Result<()> {
        let mut retries = 0;

        while retries < max_retries {
            let ps_status = Command::new("docker")
                .args(["compose", "ps"])
                .current_dir(&self.project_path)
                .output()
                .map_err(|e| {
                    let error = format!("Failed to check Docker Compose status: {}", e);
                    error!("{}", error);
                    anyhow::anyhow!(error)
                })?;

            if ps_status.status.success() {
                let output = String::from_utf8_lossy(&ps_status.stdout);
                if !output.contains("Exit") && output.contains("Up") {
                    info!("✅ All containers are up and running");

                    // Check if DATABASE_URL is set, but don't fail if it's not
                    // since E2E tests use hardcoded connection strings
                    if std::env::var("DATABASE_URL").is_ok() {
                        info!("✅ DATABASE_URL environment variable is set");
                    } else {
                        info!("ℹ️  DATABASE_URL not set, using hardcoded connection for E2E tests");
                    }

                    return Ok(());
                }
            } else {
                let error = format!("docker compose ps exited with status: {}", ps_status.status);
                warn!("{}", error);
            }

            retries += 1;
            thread::sleep(Duration::from_millis(200));
            info!(
                "⏳ Waiting for Docker Compose containers to start... ({}/{})",
                retries, max_retries
            );
        }

        Err(anyhow::anyhow!(
            "Docker containers did not start successfully within the given retries"
        ))
    }

    /// Stop the embedded server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.server_handle.take() {
            // info!("🛑 Stopping embedded RRelayer server...");
            handle.abort();

            // Wait a moment for the abort to take effect
            sleep(Duration::from_millis(100)).await;

            // Force kill any remaining RRelayer processes on port 3000
            self.force_kill_server_on_port(3000).await;

            // info!("✅ Embedded RRelayer server stopped");
        }

        // Optionally stop Docker Compose services
        // Note: We don't stop Docker by default in E2E tests as other tests might be using it
        // self.stop_docker_compose()?;

        Ok(())
    }

    /// Force kill any process listening on the given port
    async fn force_kill_server_on_port(&self, port: u16) {
        // Try to find and kill processes on the port
        if let Ok(output) = Command::new("lsof").args(["-ti", &format!("tcp:{}", port)]).output() {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.lines() {
                if let Ok(pid_num) = pid.trim().parse::<u32>() {
                    info!("🔫 Force killing process {} on port {}", pid_num, port);
                    let _ = Command::new("kill").args(["-9", &pid_num.to_string()]).output();
                }
            }
        }

        // Verify the port is now free
        sleep(Duration::from_millis(200)).await;
        match reqwest::get("http://localhost:3000/health").await {
            Ok(_) => warn!("⚠️  Port {} may still have active connections", port),
            Err(_) => info!("✅ Port {} is now free", port),
        }
    }

    /// Stop Docker Compose services (optional - not used by default)
    #[allow(dead_code)]
    fn stop_docker_compose(&self) -> Result<()> {
        info!("🛑 Stopping Docker Compose services...");

        let status = Command::new("docker")
            .args(["compose", "down"])
            .current_dir(&self.project_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| {
                let error = format!("Failed to stop Docker Compose: {}", e);
                warn!("{}", error);
                anyhow::anyhow!(error)
            })?;

        if status.success() {
            info!("✅ Docker Compose services stopped");
        } else {
            warn!("⚠️  Docker Compose stop command failed");
        }

        Ok(())
    }
}

impl Drop for EmbeddedRRelayerServer {
    fn drop(&mut self) {
        if let Some(handle) = &self.server_handle {
            handle.abort();
        }
    }
}
