use crate::{
    network::{set_networks_cache, NetworksFilterState},
    postgres::PostgresClient,
    shared::cache::Cache,
};
use std::{sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{error, info};

pub struct NetworkCacheTask {
    postgres_client: Arc<PostgresClient>,
    cache: Arc<Cache>,
}

impl NetworkCacheTask {
    pub fn new(postgres_client: Arc<PostgresClient>, cache: Arc<Cache>) -> Self {
        Self { postgres_client, cache }
    }

    pub async fn run(&mut self) {
        info!("Starting network cache refresh background task");

        self.refresh_networks_cache().await;

        let mut interval = interval(Duration::from_secs(10 * 60));

        loop {
            interval.tick().await;
            self.refresh_networks_cache().await;
        }
    }

    async fn refresh_networks_cache(&self) {
        info!("Refreshing networks cache");

        match self.postgres_client.get_networks(NetworksFilterState::All).await {
            Ok(networks) => {
                info!("Fetched {} networks from database, updating cache", networks.len());
                set_networks_cache(&self.cache, &networks).await;
                info!("Networks cache updated successfully");
            }
            Err(e) => {
                error!("Failed to refresh networks cache: {}. Cache will remain stale.", e);
            }
        }
    }
}

pub async fn run_network_cache_task(postgres_client: Arc<PostgresClient>, cache: Arc<Cache>) {
    info!("Starting network cache task");

    let mut cache_task = NetworkCacheTask::new(postgres_client, cache);

    tokio::spawn(async move {
        cache_task.run().await;
    });
}
