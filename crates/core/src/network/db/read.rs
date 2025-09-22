use std::collections::HashMap;

use crate::{
    network::types::{ChainId, Network, NetworksFilterState},
    postgres::{PostgresClient, PostgresError},
};

impl PostgresClient {
    /// Retrieves networks from the database with optional filtering.
    pub async fn get_networks(
        &self,
        filter: NetworksFilterState,
    ) -> Result<Vec<Network>, PostgresError> {
        let mut filter_sql = String::from("");
        match filter {
            NetworksFilterState::All => {}
            NetworksFilterState::Enabled => {
                filter_sql = String::from("WHERE n.disabled = FALSE");
            }
            NetworksFilterState::Disabled => {
                filter_sql = String::from("WHERE n.disabled = TRUE");
            }
        }

        let query = format!(
            r#"
            SELECT 
                n.name,
                n.chain_id,
                n.disabled,
                nn.provider_url
            FROM network.record n 
            INNER JOIN network.node nn ON nn.chain_id = n.chain_id
            {filter_sql};
            "#,
            filter_sql = filter_sql
        );

        let rows = self.query(&query, &[]).await?;

        let mut networks_map: HashMap<ChainId, Network> = HashMap::new();

        for row in rows.iter() {
            let chain_id: ChainId = row.get("chain_id");

            if let Some(network) = networks_map.get_mut(&chain_id) {
                let provider_url: String = row.get("provider_url");
                network.provider_urls.push(provider_url);
            } else {
                let network = Network {
                    name: row.get("name"),
                    chain_id,
                    disabled: row.get("disabled"),
                    provider_urls: vec![row.get("provider_url")],
                };
                networks_map.insert(chain_id, network);
            }
        }

        Ok(networks_map.into_values().collect())
    }

    /// Checks if a network exists in the database.
    pub async fn network_exists(&self, chain_id: ChainId) -> Result<bool, PostgresError> {
        let query = r#"
            SELECT EXISTS(
                SELECT 1
                FROM network.record
                WHERE chain_id = $1
            )
        "#;

        let rows = self.query(query, &[&chain_id]).await?;

        if let Some(row) = rows.first() {
            Ok(row.get::<_, bool>(0))
        } else {
            Ok(false)
        }
    }

    pub async fn network_enabled(&self, chain_id: ChainId) -> Result<bool, PostgresError> {
        let query = r#"
            SELECT EXISTS(
                SELECT 1
                FROM network.record
                WHERE chain_id = $1 AND disabled = FALSE
            )
        "#;

        let rows = self.query(query, &[&chain_id]).await?;

        if let Some(row) = rows.first() {
            Ok(row.get::<_, bool>(0))
        } else {
            Ok(false)
        }
    }
}
