use crate::{
    network::types::ChainId,
    postgres::{PostgresClient, PostgresError},
};

impl PostgresClient {
    /// Saves a new enabled network to the database.
    ///
    /// Creates a new network record and associated provider nodes in a transaction.
    /// Uses ON CONFLICT DO NOTHING to handle duplicate entries gracefully.
    ///
    /// # Arguments
    /// * `chain_id` - Unique identifier for the blockchain network
    /// * `name` - Human-readable name for the network
    /// * `provider_urls` - List of RPC provider URLs for this network
    ///
    /// # Returns
    /// * `Ok(())` - If network was successfully saved
    /// * `Err(PostgresError)` - If database transaction fails
    pub async fn save_enabled_network(
        &mut self,
        chain_id: &ChainId,
        name: &String,
        provider_urls: &Vec<String>,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        trans
            .execute(
                "INSERT INTO network.record(chain_id, name) VALUES ($1, $2) ON CONFLICT DO NOTHING;",
                &[chain_id, name],
            )
            .await?;

        for provider_url in provider_urls {
            trans
            .execute(
                "INSERT INTO network.node(chain_id, provider_url) VALUES ($1, $2) ON CONFLICT DO NOTHING;",
                &[chain_id, provider_url],
            )
            .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Disables a network by setting its disabled flag to true.
    ///
    /// Updates the network record to mark it as disabled, preventing it from
    /// being used in relay operations.
    ///
    /// # Arguments
    /// * `chain_id` - Chain ID of the network to disable
    ///
    /// # Returns
    /// * `Ok(())` - If network was successfully disabled
    /// * `Err(PostgresError)` - If database update fails
    pub async fn disable_network(&self, chain_id: ChainId) -> Result<(), PostgresError> {
        self.execute(
            "UPDATE network.record SET disabled = TRUE WHERE chain_id = $1;",
            &[&chain_id],
        )
        .await?;

        Ok(())
    }

    /// Enables a network by setting its disabled flag to false.
    ///
    /// Updates the network record to mark it as enabled, allowing it to
    /// be used in relay operations.
    ///
    /// # Arguments
    /// * `chain_id` - Chain ID of the network to enable
    ///
    /// # Returns
    /// * `Ok(())` - If network was successfully enabled
    /// * `Err(PostgresError)` - If database update fails
    pub async fn enable_network(&self, chain_id: ChainId) -> Result<(), PostgresError> {
        self.execute(
            "UPDATE network.record SET disabled = FALSE WHERE chain_id = $1;",
            &[&chain_id],
        )
        .await?;

        Ok(())
    }
}
