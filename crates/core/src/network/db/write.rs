use tokio_postgres::Transaction;

use crate::{
    network::types::ChainId,
    postgres::{PostgresClient, PostgresError},
};

impl PostgresClient {
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

    pub async fn disable_network(&self, chain_id: ChainId) -> Result<(), PostgresError> {
        self.execute(
            "UPDATE network.record SET disabled = TRUE WHERE chain_id = $1;",
            &[&chain_id],
        )
        .await?;

        Ok(())
    }

    pub async fn enable_network(&self, chain_id: ChainId) -> Result<(), PostgresError> {
        self.execute(
            "UPDATE network.record SET disabled = FALSE WHERE chain_id = $1;",
            &[&chain_id],
        )
        .await?;

        Ok(())
    }
}
