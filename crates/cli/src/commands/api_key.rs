use clap::Subcommand;
use rrelayerr_core::{
    common_types::{ApiKey, PagingContext},
    relayer::types::RelayerId,
};
use rrelayerr_sdk::SDK;

use crate::{
    authentication::handle_authenticate, commands::keystore::ProjectLocation, console::print_table,
};

#[derive(Subcommand)]
pub enum ApiKeyCommand {
    /// Add a new API key
    Add {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// List all API keys
    List {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,
    },
    /// Delete an API key
    Delete {
        /// The unique identifier of the relayer
        #[clap(required = true)]
        relayer_id: RelayerId,

        /// API key
        #[clap(required = true)]
        api_key: ApiKey,
    },
}

pub async fn handle_api_key(
    command: &ApiKeyCommand,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ApiKeyCommand::Add { relayer_id } => {
            handle_api_key_add(relayer_id, project_path, sdk).await
        }
        ApiKeyCommand::List { relayer_id } => {
            handle_api_key_list(relayer_id, project_path, sdk).await
        }
        ApiKeyCommand::Delete { relayer_id, api_key } => handle_api_key_delete(relayer_id, api_key, project_path, sdk).await,
    }
}

async fn handle_api_key_add(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let result = sdk.relayer.api_keys.create(&relayer_id).await?;

    println!("API key: {} created for relayer {}", result.api_key, relayer_id);

    Ok(())
}

async fn handle_api_key_list(
    relayer_id: &RelayerId,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    let api_keys = sdk
        .relayer
        .api_keys
        .get_all(
            relayer_id,
            &PagingContext {
                // TODO: handle paging later
                limit: 1000,
                offset: 0,
            },
        )
        .await?
        .items;

    if api_keys.is_empty() {
        println!("No API keys found for relayer {}", relayer_id);
        return Ok(());
    }

    let mut rows = Vec::new();
    for api_key in api_keys.iter() {
        rows.push(vec![api_key.to_string()]);
    }

    let headers = vec!["API Key"];

    let title = format!("{} Relayer API Keys:", api_keys.len());
    print_table(headers, rows, Some(&title), None);

    Ok(())
}

async fn handle_api_key_delete(
    relayer_id: &RelayerId,
    api_key: &ApiKey,
    project_path: &ProjectLocation,
    sdk: &mut SDK,
) -> Result<(), Box<dyn std::error::Error>> {
    handle_authenticate(sdk, "account1", project_path).await?;

    sdk.relayer.api_keys.delete(&relayer_id, api_key).await?;

    println!("API key: {} deleted for relayer {}", api_key, relayer_id);

    Ok(())
}
