use std::fmt::{Debug, Display};

use clap::{Subcommand, ValueEnum};
use rrelayerr_core::{
    authentication::types::JwtRole,
    common_types::{EvmAddress, PagingQuery},
};
use rrelayerr_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::keystore::ProjectLocation, console::print_table,
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
    fn from(role: CliJwtRole) -> Self {
        match role {
            CliJwtRole::Admin => JwtRole::Admin,
            CliJwtRole::Manager => JwtRole::Manager,
            CliJwtRole::Integrator => JwtRole::Integrator,
            CliJwtRole::ReadOnly => JwtRole::ReadOnly,
        }
    }
}

pub async fn handle_user(
    command: &UserCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        UserCommand::List => handle_list(project_path, sdk).await,
        UserCommand::Edit { address, role } => handle_edit(address, role, project_path, sdk).await,
        UserCommand::Add { address, role } => handle_add(address, role, project_path, sdk).await,
        UserCommand::Delete { address } => handle_delete(address, project_path, sdk).await,
    }
}

async fn handle_list(
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    log_users(sdk).await?;

    Ok(())
}

async fn handle_delete(
    address: &EvmAddress,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.user.delete(address).await?;

    println!("User {} deleted", address);

    Ok(())
}

async fn handle_add(
    user_address: &EvmAddress,
    role: &CliJwtRole,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let jwt_role: JwtRole = role.clone().into();

    sdk.user.add(user_address, &jwt_role).await?;

    println!("User {} added as role {}", user_address, jwt_role);

    Ok(())
}

async fn handle_edit(
    user_address: &EvmAddress,
    role: &CliJwtRole,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let jwt_role: JwtRole = role.clone().into();

    sdk.user.edit(user_address, &jwt_role).await?;

    println!("User {} role updated to {}", user_address, jwt_role);
    Ok(())
}

async fn log_users(sdk: &mut SDK) -> Result<(), Box<dyn std::error::Error>> {
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
