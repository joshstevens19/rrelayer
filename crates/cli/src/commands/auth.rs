use std::path::PathBuf;

use clap::Subcommand;
use dialoguer::Password;
use rrelayer_core::keystore::{
    KeyStorePasswordManager, KeystoreDecryptResult, PasswordError, decrypt_keystore,
};

use crate::{
    commands::error::AuthError,
    commands::keystore::{ProjectLocation, create_from_private_key},
};

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Login with a keystore account
    Login {
        /// Account name/profile to log in with
        #[clap(required = true)]
        account: String,
    },

    /// Logout from a keystore account
    Logout {
        /// Account name/profile to log out from
        #[clap(required = true)]
        account: String,
    },

    /// Create a new keystore account
    NewAccount {
        /// Account name/profile to log out from
        #[clap(required = true)]
        account: String,
    },

    /// Display account information (address and private key)
    Info {
        /// Account name/profile to display information for
        #[clap(required = true)]
        account: String,
    },

    ListAccounts,
}

/// Handles authentication command routing and execution.
///
/// Routes the authentication command to the appropriate handler function
/// based on the command type (Login, Logout, NewAccount, Info, or ListAccounts).
///
/// # Arguments
/// * `cmd` - The authentication command to execute
/// * `project_path` - Path to the project directory
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(AuthError)` - Command execution failed
pub async fn handle_auth_command(
    cmd: &AuthCommand,
    project_path: PathBuf,
) -> Result<(), AuthError> {
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
        AuthCommand::Info { account } => {
            show_account_info(account, ProjectLocation::new(project_path))?;
        }
        AuthCommand::ListAccounts => {
            list_accounts(ProjectLocation::new(project_path))?;
        }
    }

    Ok(())
}

/// Logs in to a keystore account.
///
/// Prompts the user for their password, decrypts the keystore to verify
/// credentials, and stores the password in the password manager for future use.
///
/// # Arguments
/// * `account` - Name of the account to log in with
/// * `project_location` - Project location containing keystore files
///
/// # Returns
/// * `Ok(())` - Login successful
/// * `Err(AuthError)` - Login failed due to invalid credentials or other error
fn login(account: &str, project_location: ProjectLocation) -> Result<(), AuthError> {
    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name())?;

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
        Err(_) => Err(AuthError::InvalidCredentials),
    }
}

/// Logs out from a keystore account.
///
/// Removes the stored password from the password manager, requiring
/// re-authentication for future operations.
///
/// # Arguments
/// * `account` - Name of the account to log out from
/// * `project_location` - Project location containing keystore files
///
/// # Returns
/// * `Ok(())` - Logout successful or account was not logged in
/// * `Err(AuthError)` - Logout failed due to password manager error
fn logout(account: &str, project_location: ProjectLocation) -> Result<(), AuthError> {
    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name())?;

    match password_manager.delete(account) {
        Ok(_) => {
            println!("Successfully logged out from account '{}'", account);
            Ok(())
        }
        Err(PasswordError::NotFound) => {
            println!("Not logged in as '{}'", account);
            Ok(())
        }
        Err(e) => Err(AuthError::PasswordManager(e)),
    }
}

/// Creates a new keystore account.
///
/// Generates a new random private key and creates an encrypted keystore
/// file for the account.
///
/// # Arguments
/// * `account` - Name for the new account
/// * `project_location` - Project location where keystore will be created
///
/// # Returns
/// * `Ok(())` - Account created successfully
/// * `Err(AuthError)` - Account creation failed
fn new_account(account: &str, project_location: ProjectLocation) -> Result<(), AuthError> {
    create_from_private_key(&None, true, account, project_location, None)?;

    Ok(())
}

/// Displays account information including address and private key.
///
/// Decrypts the keystore and shows the account's address and private key
/// or mnemonic phrase. Prompts for password if not already logged in.
///
/// # Arguments
/// * `account` - Name of the account to show information for
/// * `project_location` - Project location containing keystore files
///
/// # Returns
/// * `Ok(())` - Account information displayed successfully
/// * `Err(AuthError)` - Failed to decrypt keystore or show information
fn show_account_info(account: &str, project_location: ProjectLocation) -> Result<(), AuthError> {
    let password_manager = KeyStorePasswordManager::new(&project_location.get_project_name())?;

    let password = match password_manager.load(account) {
        Ok(password) => password,
        Err(PasswordError::NotFound) => {
            let password = Password::new()
                .with_prompt(format!("Enter password for account '{}'", account))
                .interact()?;

            match decrypt_keystore(&project_location.get_account_keystore(account), &password) {
                Ok(_) => password,
                Err(_) => return Err(AuthError::InvalidCredentials),
            }
        }
        Err(e) => return Err(AuthError::PasswordManager(e)),
    };

    let result = decrypt_keystore(&project_location.get_account_keystore(account), &password)?;

    match result {
        KeystoreDecryptResult::PrivateKey { hex_key, address, .. } => {
            println!("Account information for '{}':", account);
            println!("Address: {}", address);
            println!("Private key: {}", hex_key);
        }
        KeystoreDecryptResult::Mnemonic { phrase, address } => {
            println!("Account information for '{}':", account);
            println!("Address: {}", address);
            println!("Mnemonic phrase: {}", phrase);
            println!("Note: This account uses a mnemonic phrase rather than a direct private key.");
        }
    }

    Ok(())
}

/// Lists all logged-in accounts for the current project.
///
/// Shows all accounts that have stored passwords in the password manager,
/// indicating which accounts are currently logged in.
///
/// # Arguments
/// * `project_location` - Project location to check for logged-in accounts
///
/// # Returns
/// * `Ok(())` - Account list displayed successfully
/// * `Err(AuthError)` - Failed to retrieve account list
fn list_accounts(project_location: ProjectLocation) -> Result<(), AuthError> {
    let project_name = project_location.get_project_name();
    let password_manager = KeyStorePasswordManager::new(&project_name)?;

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
