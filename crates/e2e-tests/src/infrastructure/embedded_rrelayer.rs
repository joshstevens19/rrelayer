use anyhow::Result;
use rrelayer_core::{start, StartError};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct EmbeddedRRelayerServer {
    project_path: PathBuf,
    server_handle: Option<tokio::task::JoinHandle<()>>,
    server_process: Option<Child>,
    use_separate_process: bool,
}

impl EmbeddedRRelayerServer {
    pub fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            server_handle: None,
            server_process: None,
            use_separate_process: false, // Default to embedded mode for debugging
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("[START] Starting RRelayer server...");

        self.start_docker_compose()?;
        self.wait_for_postgres().await?;
        self.reset_database().await?;

        if self.use_separate_process {
            self.start_as_separate_process().await?;
        } else {
            self.start_as_embedded().await?;
        }

        self.wait_for_ready().await?;

        // Give the server a moment to fully initialize after health check
        sleep(Duration::from_millis(500)).await;

        info!("[SUCCESS] RRelayer server is ready!");
        Ok(())
    }

    async fn start_as_separate_process(&mut self) -> Result<()> {
        info!("[START] Starting RRelayer as separate process for complete isolation...");

        // Start RRelayer as a separate process using cargo run
        let mut child = Command::new("cargo")
            .args(["run", "--bin", "rrelayer"])
            .current_dir(&self.project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start RRelayer process: {}", e))?;

        // Store the process handle for later termination
        self.server_process = Some(child);

        Ok(())
    }

    async fn start_as_embedded(&mut self) -> Result<()> {
        info!("[START] Starting RRelayer as embedded server...");

        let project_path = self.project_path.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::run_server(&project_path).await {
                error!("[ERROR] RRelayer server failed: {}", e);
            }
        });

        self.server_handle = Some(handle);

        Ok(())
    }

    async fn wait_for_ready(&self) -> Result<()> {
        let mut retries = 30;

        while retries > 0 {
            match reqwest::get("http://localhost:3000/health").await {
                Ok(response) if response.status().is_success() => {
                    info!("[SUCCESS] RRelayer health check passed");
                    return Ok(());
                }
                Ok(response) => {
                    warn!("[WARNING]  Health check returned {}, retrying...", response.status());
                }
                Err(e) => {
                    warn!("[WARNING]  Health check failed: {}, retrying...", e);
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
            retries -= 1;
        }

        Err(anyhow::anyhow!("RRelayer server did not become ready after 60 seconds"))
    }

    async fn run_server(project_path: &PathBuf) -> Result<(), StartError> {
        info!("Starting up the embedded RRelayer server");
        start(project_path).await
    }

    async fn reset_database(&self) -> Result<()> {
        info!("🗑️  Resetting database schemas for fresh E2E test run...");

        let connection_string =
            std::env::var("DATABASE_URL").expect("DATABASE_URL needs to be set");

        let schemas_to_drop =
            vec!["public", "rate_limit", "signing", "network", "relayer", "webhooks"];

        match tokio_postgres::connect(&connection_string, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        error!("Database connection error: {}", e);
                    }
                });

                for schema in &schemas_to_drop {
                    let drop_query = format!("DROP SCHEMA IF EXISTS {} CASCADE", schema);
                    match client.execute(&drop_query, &[]).await {
                        Ok(_) => info!("[SUCCESS] Dropped schema: {}", schema),
                        Err(e) => warn!("[WARNING]  Failed to drop schema {}: {}", schema, e),
                    }
                }

                match client.execute("CREATE SCHEMA public", &[]).await {
                    Ok(_) => info!("[SUCCESS] Recreated public schema"),
                    Err(e) => warn!("[WARNING]  Failed to recreate public schema: {}", e),
                }

                info!("[SUCCESS] Database reset complete");
            }
            Err(e) => {
                warn!(
                    "[WARNING]  Could not connect to database for reset: {}. Continuing anyway...",
                    e
                );
                warn!("   Make sure PostgreSQL is running with the correct credentials");
            }
        }

        Ok(())
    }

    fn start_docker_compose(&self) -> Result<()> {
        info!("🐳 Starting Docker Compose services from: {:?}", self.project_path);

        if !self.project_path.exists() {
            return Err(anyhow::anyhow!("Project path does not exist: {:?}", self.project_path));
        }

        // Check if docker-compose.yml exists
        let compose_file = self.project_path.join("docker-compose.yml");
        if !compose_file.exists() {
            return Err(anyhow::anyhow!("docker-compose.yml not found at: {:?}", compose_file));
        }

        info!("🐳 Running: docker compose up -d");
        let output = Command::new("docker")
            .args(["compose", "up", "-d"])
            .current_dir(&self.project_path)
            .output()
            .map_err(|e| {
                let error = format!(
                    "Docker command could not be executed. Make sure Docker is running: {}",
                    e
                );
                error!("{}", error);
                anyhow::anyhow!(error)
            })?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error = format!(
                "Docker Compose failed to start containers.\nSTDOUT: {}\nSTDERR: {}",
                stdout, stderr
            );
            error!("{}", error);
            return Err(anyhow::anyhow!(error));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("🐳 Docker Compose output: {}", stdout);

        info!("🐳 Checking if PostgreSQL container is running...");
        self.check_docker_compose_status(50)
    }

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
                info!("🐳 Docker Compose status:\n{}", output);

                if !output.contains("Exit") && output.contains("Up") {
                    info!("[SUCCESS] All containers are up and running");

                    // Check if DATABASE_URL is set, but don't fail if it's not
                    // since E2E tests use hardcoded connection strings
                    if std::env::var("DATABASE_URL").is_ok() {
                        info!("[SUCCESS] DATABASE_URL environment variable is set");
                    } else {
                        info!("ℹ️  DATABASE_URL not set, using hardcoded connection for E2E tests");
                    }

                    return Ok(());
                } else if output.contains("Exit") {
                    warn!("[WARNING] Some containers have exited: {}", output);
                } else {
                    info!("[WAIT] Containers are still starting...");
                }
            } else {
                let stderr = String::from_utf8_lossy(&ps_status.stderr);
                let error = format!("docker compose ps failed: {}", stderr);
                warn!("{}", error);
            }

            retries += 1;
            thread::sleep(Duration::from_millis(200));
            info!(
                "[WAIT] Waiting for Docker Compose containers to start... ({}/{})",
                retries, max_retries
            );
        }

        Err(anyhow::anyhow!(
            "Docker containers did not start successfully within {} retries",
            max_retries
        ))
    }

    async fn wait_for_postgres(&self) -> Result<()> {
        info!("Waiting for PostgreSQL to be ready...");

        let connection_string = "postgres://postgres:rrelayer@localhost:5447/postgres";
        let mut retries = 30; // 30 attempts with 1 second each = 30 seconds max

        while retries > 0 {
            match tokio_postgres::connect(connection_string, tokio_postgres::NoTls).await {
                Ok((client, connection)) => {
                    // Spawn the connection task
                    let connection_task = tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            error!("PostgreSQL connection error: {}", e);
                        }
                    });

                    // Try to execute a simple query
                    match client.execute("SELECT 1", &[]).await {
                        Ok(_) => {
                            info!("[SUCCESS] PostgreSQL is ready!");
                            // Clean up the connection
                            connection_task.abort();
                            return Ok(());
                        }
                        Err(e) => {
                            warn!("PostgreSQL query failed: {}, retrying...", e);
                            connection_task.abort();
                        }
                    }
                }
                Err(e) => {
                    info!("Attempt {}/30 - PostgreSQL not ready yet... ({})", 31 - retries, e);
                }
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
            retries -= 1;
        }

        Err(anyhow::anyhow!("PostgreSQL did not become ready after 30 seconds"))
    }

    /// Stop the embedded server
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_with_docker_cleanup(false).await
    }

    pub async fn stop_with_docker_cleanup(&mut self, cleanup_docker: bool) -> Result<()> {
        info!("[STOP] Shutting down RRelayer server...");

        if self.use_separate_process {
            self.stop_separate_process().await?;
        } else {
            self.stop_embedded_server().await?;
        }

        // Optionally stop Docker Compose services and perform additional cleanup
        if cleanup_docker {
            info!("[CLEANUP] Stopping Docker Compose services for multi-provider isolation...");
            self.stop_docker_compose()?;

            // Give Docker time to fully stop
            sleep(Duration::from_secs(2)).await;
        } else {
            // Note: We don't stop Docker by default in E2E tests as other tests might be using it
        }

        Ok(())
    }

    async fn stop_separate_process(&mut self) -> Result<()> {
        if let Some(mut process) = self.server_process.take() {
            info!("[STOP] Terminating RRelayer process for complete isolation...");

            // Try graceful termination first (SIGTERM)
            if let Err(e) = process.kill() {
                warn!("[WARNING] Failed to terminate RRelayer process gracefully: {}", e);
            }

            // Wait for process to exit
            match process.wait() {
                Ok(status) => {
                    info!("[SUCCESS] RRelayer process exited with status: {}", status);
                }
                Err(e) => {
                    warn!("[WARNING] Error waiting for RRelayer process: {}", e);
                }
            }

            // Verify the server has stopped by checking health endpoint
            self.wait_for_server_stop().await;

            info!("[SUCCESS] RRelayer process shutdown complete");
        }
        Ok(())
    }

    async fn stop_embedded_server(&mut self) -> Result<()> {
        if let Some(handle) = self.server_handle.take() {
            info!("[STOP] Shutting down embedded RRelayer server...");

            // Immediately abort the server handle to stop all tasks
            handle.abort();

            // Wait for the server to stop responding to health checks
            self.wait_for_server_stop().await;

            // Force kill any remaining processes on port 3000 to ensure complete cleanup
            info!("[CLEANUP] Force killing any remaining processes on port 3000...");
            self.force_kill_server_on_port(3000).await;

            info!("[SUCCESS] Embedded RRelayer server shutdown complete");
        }
        Ok(())
    }

    async fn wait_for_server_stop(&self) {
        let mut retries = 5; // Only wait 5 seconds for E2E tests

        while retries > 0 {
            match reqwest::get("http://localhost:3000/health").await {
                Err(_) => {
                    info!("[SUCCESS] RRelayer server has stopped responding");
                    return;
                }
                Ok(_) => {
                    info!(
                        "[WAIT] Waiting for RRelayer server to stop... ({} retries left)",
                        retries
                    );
                    sleep(Duration::from_secs(1)).await;
                    retries -= 1;
                }
            }
        }

        warn!("[WARNING] RRelayer server did not stop gracefully after 5 seconds");
    }

    async fn force_kill_server_on_port(&self, port: u16) {
        if let Ok(output) = Command::new("lsof").args(["-ti", &format!("tcp:{}", port)]).output() {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid in pids.lines() {
                if let Ok(pid_num) = pid.trim().parse::<u32>() {
                    info!("🔫 Force killing process {} on port {}", pid_num, port);
                    let _ = Command::new("kill").args(["-9", &pid_num.to_string()]).output();
                }
            }
        }

        sleep(Duration::from_millis(200)).await;
        match reqwest::get("http://localhost:3000/health").await {
            Ok(_) => warn!("[WARNING]  Port {} may still have active connections", port),
            Err(_) => info!("[SUCCESS] Port {} is now free", port),
        }
    }

    fn stop_docker_compose(&self) -> Result<()> {
        info!("[STOP] Stopping Docker Compose services...");

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
            info!("[SUCCESS] Docker Compose services stopped");
        } else {
            warn!("[WARNING]  Docker Compose stop command failed");
        }

        Ok(())
    }
}

impl Drop for EmbeddedRRelayerServer {
    fn drop(&mut self) {
        if let Some(handle) = &self.server_handle {
            handle.abort();
        }
        if let Some(mut process) = self.server_process.take() {
            let _ = process.kill();
        }
    }
}
