mod automatic_top_up_task;

use crate::background_tasks::automatic_top_up_task::run_automatic_top_up_task;
use crate::gas::blob_gas_oracle::{blob_gas_oracle, BlobGasOracleCache};
use crate::gas::gas_oracle::{gas_oracle, GasOracleCache};
use crate::provider::EvmProvider;
use crate::{rrelayer_info, PostgresClient, SetupConfig};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub async fn run_background_tasks(
    config: &SetupConfig,
    gas_oracle_cache: Arc<Mutex<GasOracleCache>>,
    blob_gas_oracle_cache: Arc<Mutex<BlobGasOracleCache>>,
    providers: Arc<Vec<EvmProvider>>,
    postgres_client: Arc<PostgresClient>,
) {
    rrelayer_info!("Starting background tasks");

    let gas_oracle_task = gas_oracle(Arc::clone(&providers), gas_oracle_cache);
    let blob_gas_oracle_task = blob_gas_oracle(Arc::clone(&providers), blob_gas_oracle_cache);
    let top_up_task = run_automatic_top_up_task(config.clone(), postgres_client, providers);

    tokio::join!(gas_oracle_task, blob_gas_oracle_task, top_up_task);

    rrelayer_info!("Background tasks spawned up");
}
