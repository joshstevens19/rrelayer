use clap::Subcommand;

#[derive(Subcommand)]
pub enum SignCommand {
    /// Sign a text message
    Text,
    /// Sign typed data
    TypedData,
}

pub async fn handle_sign(
    relayer_id: &str,
    command: &SignCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        SignCommand::Text => handle_sign_text(relayer_id),
        SignCommand::TypedData => handle_sign_typed_data(relayer_id),
    }
}

fn handle_sign_text(relayer_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Enter the message to sign:");
    let mut message = String::new();
    std::io::stdin().read_line(&mut message)?;
    let message = message.trim();

    println!("Signing message for relayer: {}", relayer_id);
    // TODO: Implement message signing logic
    println!("Message: {}", message);

    Ok(())
}

fn handle_sign_typed_data(relayer_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Enter the typed data to sign (JSON format):");
    let mut typed_data = String::new();
    std::io::stdin().read_line(&mut typed_data)?;
    let typed_data = typed_data.trim();

    println!("Signing typed data for relayer: {}", relayer_id);
    // TODO: Implement typed data validation and signing logic
    println!("Typed Data: {}", typed_data);

    Ok(())
}
