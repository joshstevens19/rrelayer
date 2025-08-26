use std::fmt::Debug;

use clap::{Subcommand, ValueEnum};
use rrelayer_core::{
    authentication::types::JwtRole,
    common_types::{EvmAddress, PagingQuery},
};
use rrelayer_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::error::UserError,
    commands::keystore::ProjectLocation, console::print_table,
};

#[derive(Subcommand)]
pub enum UserCommand {
    /// List all users
    List,
    /// Edit user role
    Edit {
        /// The address to edit
        #[clap(required = true)]
        address: EvmAddress,

        /// The role to assign
        #[clap(required = true, value_enum)]
        role: CliJwtRole,
    },
    /// Add a new user
    Add {
        /// The address to add
        #[clap(required = true)]
        address: EvmAddress,

        /// The role to assign
        #[clap(required = true, value_enum)]
        role: CliJwtRole,
    },
    /// Delete a user
    Delete {
        /// The address to delete
        #[clap(required = true)]
        address: EvmAddress,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum CliJwtRole {
    Admin,
    Manager,
    Integrator,
    ReadOnly,
}

impl From<CliJwtRole> for JwtRole {
    /// Converts a CLI JWT role enum to the core JWT role enum.
    ///
    /// # Arguments
    /// * `role` - The CLI JWT role to convert
    ///
    /// # Returns
    /// * The corresponding core JWT role
    fn from(role: CliJwtRole) -> Self {
        match role {
            CliJwtRole::Admin => JwtRole::Admin,
            CliJwtRole::Manager => JwtRole::Manager,
            CliJwtRole::Integrator => JwtRole::Integrator,
            CliJwtRole::ReadOnly => JwtRole::ReadOnly,
        }
    }
}

/// Handles user management commands by dispatching to the appropriate handler function.
///
/// Routes user commands to their respective handlers for listing, adding, editing,
/// or deleting users and their roles.
///
/// # Arguments
/// * `command` - The user management command to execute
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Command executed successfully
/// * `Err(UserError)` - Command execution failed
pub async fn handle_user(
    command: &UserCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), UserError> {
    match command {
        UserCommand::List => handle_list(project_path, sdk).await,
        UserCommand::Edit { address, role } => handle_edit(address, role, project_path, sdk).await,
        UserCommand::Add { address, role } => handle_add(address, role, project_path, sdk).await,
        UserCommand::Delete { address } => handle_delete(address, project_path, sdk).await,
    }
}

/// Lists all users and their roles in a formatted table.
///
/// Authenticates the user and retrieves all users, then displays them
/// in a table format with their addresses and assigned roles.
///
/// # Arguments
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Users listed successfully
/// * `Err(UserError)` - Authentication failed or user retrieval failed
async fn handle_list(project_path: &ProjectLocation, sdk: &mut SDK) -> Result<(), UserError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    log_users(sdk).await?;

    Ok(())
}

/// Deletes a user from the system.
///
/// Authenticates the user and removes the specified user address from
/// the system, revoking their access.
///
/// # Arguments
/// * `address` - The Ethereum address of the user to delete
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - User deleted successfully
/// * `Err(UserError)` - Authentication failed or deletion failed
async fn handle_delete(
    address: &EvmAddress,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), UserError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.user.delete(address).await?;

    println!("User {} deleted", address);

    Ok(())
}

/// Adds a new user to the system with the specified role.
///
/// Authenticates the user and adds the specified address with the given
/// role to the system, granting them appropriate access permissions.
///
/// # Arguments
/// * `user_address` - The Ethereum address of the user to add
/// * `role` - The role to assign to the new user
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - User added successfully
/// * `Err(UserError)` - Authentication failed or user addition failed
async fn handle_add(
    user_address: &EvmAddress,
    role: &CliJwtRole,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), UserError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let jwt_role: JwtRole = role.clone().into();

    sdk.user.add(user_address, &jwt_role).await?;

    println!("User {} added as role {}", user_address, jwt_role);

    Ok(())
}

/// Updates an existing user's role.
///
/// Authenticates the user and updates the specified user's role to the
/// new role, changing their access permissions accordingly.
///
/// # Arguments
/// * `user_address` - The Ethereum address of the user to update
/// * `role` - The new role to assign to the user
/// * `project_path` - The project location containing configuration and keystores
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - User role updated successfully
/// * `Err(UserError)` - Authentication failed or role update failed
async fn handle_edit(
    user_address: &EvmAddress,
    role: &CliJwtRole,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), UserError> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let jwt_role: JwtRole = role.clone().into();

    sdk.user.edit(user_address, &jwt_role).await?;

    println!("User {} role updated to {}", user_address, jwt_role);
    Ok(())
}

/// Retrieves and displays users in a formatted table.
///
/// Fetches all users from the API and displays them in a table format
/// with columns for address and role. Includes pagination context
/// but currently retrieves all users.
///
/// # Arguments
/// * `sdk` - Mutable reference to the SDK for making API calls
///
/// # Returns
/// * `Ok(())` - Users displayed successfully
/// * `Err(UserError)` - Failed to fetch users from API
async fn log_users(sdk: &mut SDK) -> Result<(), UserError> {
    let users = sdk
        .user
        .get(&PagingQuery {
            // don't handle paging just yet as probably not required
            limit: 1000,
            offset: 0,
        })
        .await?
        .items;

    let mut rows = Vec::new();
    for user in users.iter() {
        rows.push(vec![user.address.hex(), user.role.to_string()]);
    }

    let headers = vec!["Address", "Role"];

    let title = format!("{} Users:", users.len());
    let footer = "Roles can be admin, manager, integrator and readonly";

    print_table(headers, rows, Some(&title), Some(footer));

    Ok(())
}
