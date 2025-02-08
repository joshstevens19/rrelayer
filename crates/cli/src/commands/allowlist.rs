use clap::Subcommand;

#[derive(Subcommand)]
pub enum AllowlistCommand {
    /// Add an address to allowlist
    Add { address: String },
    /// List all allowlisted addresses
    List,
    /// Delete an address from allowlist
    Delete { address: String },
}

pub fn handle_allowlist(
    relayer_id: &str,
    command: &AllowlistCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        AllowlistCommand::Add { address } => handle_allowlist_add(relayer_id, address),
        AllowlistCommand::List => handle_allowlist_list(relayer_id),
        AllowlistCommand::Delete { address } => handle_allowlist_delete(relayer_id, address),
    }
}

fn handle_allowlist_add(relayer_id: &str, address: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Adding address {} to allowlist for relayer: {}", address, relayer_id);
    // TODO: Implement address validation and storage logic
    Ok(())
}

fn handle_allowlist_list(relayer_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Listing allowlisted addresses for relayer: {}", relayer_id);
    // TODO: Implement address listing logic
    Ok(())
}

fn handle_allowlist_delete(
    relayer_id: &str,
    address: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Deleting address {} from allowlist for relayer: {}", address, relayer_id);
    // TODO: Implement address deletion logic
    Ok(())
}
