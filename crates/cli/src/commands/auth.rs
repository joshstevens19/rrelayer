use clap::Subcommand;

#[derive(Subcommand)]
pub enum AuthCommand {
    Status,
}

pub async fn handle_auth_command(cmd: &AuthCommand) -> () {
    match cmd {
        AuthCommand::Status => {
            status().await;
        }
    }

    ()
}

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
