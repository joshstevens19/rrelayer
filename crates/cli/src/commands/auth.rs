use std::path::PathBuf;

use clap::Subcommand;
use dialoguer::Password;
use rrelayerr_core::keystore::{KeyStorePasswordManager, PasswordError, decrypt_keystore};

use crate::commands::keystore::{ProjectLocation, create_from_private_key};

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

    ListAccounts,
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
            logout(account, ProjectLocation::new(project_path))?;
        }
        AuthCommand::ListAccounts => {
            list_accounts(ProjectLocation::new(project_path))?;
        }
    }

    Ok(())
}

fn login(
    account: &str,
    project_location: ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name());

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

fn logout(
    account: &str,
    project_location: ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name());

    match password_manager.delete(account) {
        Ok(_) => {
            println!("Successfully logged out from account '{}'", account);
            Ok(())
        }
        Err(PasswordError::NotFound) => {
            println!("Not logged in as '{}'", account);
            Ok(())
        }
        Err(e) => Err(format!("Error during logout: {}", e).into()),
    }
}

fn new_account(
    account: &str,
    project_location: ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
    create_from_private_key(&None, true, account, project_location, None)?;

    Ok(())
}

fn list_accounts(project_location: ProjectLocation) -> Result<(), Box<dyn std::error::Error>> {
    let project_name = project_location.get_project_name();
    let password_manager = KeyStorePasswordManager::new(&project_name);

    let logged_in_accounts = password_manager.list_accounts()?;

    if logged_in_accounts.is_empty() {
        println!("You are not logged in with any accounts for project '{}'", project_name);
    } else {
        println!("You are logged in with the following accounts for project '{}':", project_name);
        for account in logged_in_accounts {
            println!("  - {}", account);
        }
    }

    Ok(())
}
