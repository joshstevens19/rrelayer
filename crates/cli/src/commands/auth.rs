use clap::Subcommand;

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Check authentication status
    Status,
}

/// Handles authentication command routing and execution.
///
/// Routes the authentication command to the appropriate handler function
/// based on the command type.
///
/// # Arguments
/// * `cmd` - The authentication command to execute
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(AuthError)` - Command execution failed
pub async fn handle_auth_command(cmd: &AuthCommand) -> () {
    match cmd {
        AuthCommand::Status => {
            status().await;
        }
    }

    ()
}

/// Shows the current authentication status for basic auth.
///
/// Displays information about the configured basic auth credentials
/// without revealing the actual password.
///
/// # Returns
/// * `Ok(())` - Status displayed successfully
/// * `Err(AuthError)` - Failed to check authentication status
async fn status() -> () {
    use std::env;

    println!("Basic Authentication Status:");

    match env::var("RRELAYER_AUTH_USERNAME") {
        Ok(username) => println!("✅ Username: {}", username),
        Err(_) => println!("❌ Username: Not configured (RRELAYER_AUTH_USERNAME)"),
    }

    match env::var("RRELAYER_AUTH_PASSWORD") {
        Ok(_) => println!("✅ Password: Configured"),
        Err(_) => println!("❌ Password: Not configured (RRELAYER_AUTH_PASSWORD)"),
    }

    println!("\nNote: With basic auth, these credentials are used for all API access.");

    ()
}
