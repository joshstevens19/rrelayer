use std::sync::Arc;

use crate::{
    app_state::AppState,
    network::ChainId,
    provider::EvmProvider,
    relayer::{cache::invalidate_relayer_cache, Relayer},
    shared::HttpError,
    transaction::{queue_system::TransactionsQueueSetup, NonceManager},
};

/// Starts the transaction queue for a relayer and initializes it with the current nonce.
///
/// This is shared logic used by both `create_relayer` and `import_relayer` endpoints.
pub async fn start_relayer_queue(
    state: &Arc<AppState>,
    relayer: Relayer,
    provider: &EvmProvider,
    chain_id: &ChainId,
) -> Result<(), HttpError> {
    let current_nonce = provider.get_nonce(&relayer).await?;

    let network_config = state.network_configs.iter().find(|config| &config.chain_id == chain_id);

    let gas_bump_config =
        network_config.map(|config| config.gas_bump_blocks_every.clone()).unwrap_or_default();

    let max_gas_price_multiplier =
        network_config.map(|config| config.max_gas_price_multiplier).unwrap_or(2);

    let relayer_id = relayer.id;

    // Start the transaction queue for this relayer
    state
        .transactions_queues
        .lock()
        .await
        .add_new_relayer(
            TransactionsQueueSetup::new(
                relayer,
                provider.clone(),
                NonceManager::new(current_nonce),
                Default::default(),
                Default::default(),
                Default::default(),
                state.safe_proxy_manager.clone(),
                gas_bump_config,
                max_gas_price_multiplier,
            ),
            state.transactions_queues.clone(),
        )
        .await?;

    invalidate_relayer_cache(&state.cache, &relayer_id).await;

    Ok(())
}
