use std::{sync::Arc, time::Duration};

use alloy::primitives::utils::{format_ether, parse_ether};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::common_types::EvmAddress;
use crate::{
    network::ChainId, postgres::PostgresClient, provider::EvmProvider, webhooks::WebhookManager,
};

fn get_minimum_balance_threshold(chain_id: &ChainId) -> u128 {
    if chain_id.u64() == 1 {
        let value = parse_ether("0.005").expect("Failed to parse native token threshold");
        value.to::<u128>()
    } else {
        let value = parse_ether("0.001").expect("Failed to parse native token threshold");
        value.to::<u128>()
    }
}

pub async fn balance_monitor(
    providers: Arc<Vec<EvmProvider>>,
    db: Arc<PostgresClient>,
    webhook_manager: Option<Arc<Mutex<WebhookManager>>>,
) {
    info!("Starting balance monitoring background task");

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(600));

        loop {
            interval.tick().await;

            info!("Starting balance monitoring check");

            for provider in providers.iter() {
                if let Err(e) = check_balances_for_chain(provider, &db, &webhook_manager).await {
                    error!("Failed to check balances for chain {}: {}", provider.chain_id, e);
                }
            }

            info!("Completed balance monitoring check");
        }
    });

    info!("Started balance monitoring background task");
}

async fn check_balances_for_chain(
    provider: &EvmProvider,
    db: &Arc<PostgresClient>,
    webhook_manager: &Option<Arc<Mutex<WebhookManager>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let chain_id = provider.chain_id;
    let min_balance_wei = get_minimum_balance_threshold(&chain_id);
    let min_balance_formatted = format_ether(min_balance_wei);

    info!("Checking balances for chain {} (minimum: {} ETH)", chain_id, min_balance_formatted);

    let relayers = db.get_all_relayers_for_chain(&chain_id).await?;

    for relayer in relayers {
        match provider.get_balance(&relayer.address).await {
            Ok(balance_wei) => {
                let balance_u128: u128 = balance_wei.to::<u128>();

                if balance_u128 < min_balance_wei {
                    let balance_formatted = format_ether(balance_wei);

                    warn!(
                        "Low balance warning: relayer {} (ID: {}) on chain {} has balance {} ETH (minimum recommended: {} ETH)",
                        relayer.address,
                        relayer.id,
                        chain_id,
                        balance_formatted,
                        min_balance_formatted
                    );

                    if let Some(webhook_manager) = webhook_manager {
                        send_low_balance_webhook(
                            webhook_manager,
                            &relayer.id.to_string(),
                            &relayer.address,
                            &chain_id,
                            balance_u128,
                            min_balance_wei,
                            balance_formatted,
                            min_balance_formatted.clone(),
                        )
                        .await;
                    }
                } else {
                    let balance_formatted = format_ether(balance_wei);
                    info!(
                        "Balance OK: relayer {} (ID: {}) on chain {} has balance {} ETH",
                        relayer.address, relayer.id, chain_id, balance_formatted
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to get balance for relayer {} (ID: {}) on chain {}: {}",
                    relayer.address, relayer.id, chain_id, e
                );
            }
        }
    }

    Ok(())
}

async fn send_low_balance_webhook(
    webhook_manager: &Arc<Mutex<WebhookManager>>,
    relayer_id: &str,
    address: &EvmAddress,
    chain_id: &ChainId,
    current_balance: u128,
    minimum_balance: u128,
    current_balance_formatted: String,
    minimum_balance_formatted: String,
) {
    let manager = webhook_manager.lock().await;

    manager
        .queue_low_balance_webhook(
            relayer_id,
            address,
            *chain_id,
            current_balance,
            minimum_balance,
            current_balance_formatted,
            minimum_balance_formatted,
        )
        .await;
}
