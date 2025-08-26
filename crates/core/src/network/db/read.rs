use std::collections::HashMap;

use crate::{
    network::types::{ChainId, Network, NetworksFilterState},
    postgres::{PostgresClient, PostgresError},
};

impl PostgresClient {
    /// Retrieves networks from the database with optional filtering.
    ///
    /// Fetches network data including provider URLs based on the specified filter.
    /// Networks with multiple provider URLs are consolidated into single Network objects.
    ///
    /// # Arguments
    /// * `filter` - Specifies which networks to retrieve (All, Enabled, or Disabled)
    ///
    /// # Returns
    /// * `Ok(Vec<Network>)` - List of networks matching the filter criteria
    /// * `Err(PostgresError)` - If database query fails
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
}
