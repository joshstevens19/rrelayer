use std::path::PathBuf;

use clap::Subcommand;
use dialoguer::Password;
use rrelayerr_core::keystore::{decrypt_keystore, KeyStorePasswordManager};

use crate::commands::keystore::{create_from_private_key, ProjectLocation};

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Login with a keystore account
    Login {
        /// Account name/profile to log in with
        #[clap(long)]
        account: String,
    },

    /// Logout from a keystore account
    Logout {
        /// Account name/profile to log out from
        #[clap(long)]
        account: String,
    },

    /// Create a new keystore account
    NewAccount {
        /// Account name/profile to log out from
        #[clap(long)]
        account: String,
    },
}

pub async fn handle_auth_command(
    cmd: &AuthCommand,
    project_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        AuthCommand::Login { account } => {
            login(account, ProjectLocation::new(project_path))?;
        }
        AuthCommand::NewAccount { account } => {
            new_account(account, ProjectLocation::new(project_path))?;
        }
        AuthCommand::Logout { account } => {
            unimplemented!("Logout command not implemented yet")
        }
    }

    Ok(())
}

fn login(
    account: &str,
    project_location: ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
    let password_manager = KeyStorePasswordManager::new(&project_location.setup_config()?.name);

    if password_manager.load(account).is_ok() {
        println!("Already logged in as '{}'", account);
        return Ok(());
    }

    let password = Password::new()
        .with_prompt(format!("Enter password for account '{}'", account))
        .interact()?;

    match decrypt_keystore(&project_location.get_account_keystore(account), &password) {
        Ok(_) => {
            password_manager.save(account, &password)?;
            println!("Successfully logged in as '{}'", account);
            Ok(())
        }
        Err(_) => Err("Invalid password or keystore not found".into()),
    }
}

fn new_account(
    account: &str,
    project_location: ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
    create_from_private_key(&None, true, account, project_location)?;

    Ok(())
}
