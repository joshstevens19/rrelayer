use std::{fs, path::Path, process::Command};

use clap::Args;

#[derive(Args)]
pub struct StopArgs {}

pub async fn handle_stop() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we're in a relayer project directory
    if !Path::new("rrelayer.yaml").exists() {
        return Err(
            "Not in a relayer project directory. Please run this command from your project root."
                .into(),
        );
    }

    // Read the config
    let config = fs::read_to_string("rrelayer.yaml")?;

    println!("Stopping relayer...");

    // Here you would implement the actual relayer stop logic
    // For example, if using Docker:
    if config.contains("docker_support: true") {
        Command::new("docker").args(["compose", "down"]).status()?;
        println!("Relayer stopped in Docker container");
    } else {
        // Implement native stop logic here
        println!("Relayer stopped natively");
    }

    Ok(())
}
