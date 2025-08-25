use std::error::Error;
use thiserror::Error;

use crate::{
    gas::types::GasPrice,
    network::types::ChainId,
    postgres::{PostgresClient, PostgresError},
    provider::EvmProvider,
    relayer::types::{Relayer, RelayerId},
    shared::common_types::EvmAddress,
};

#[derive(Error, Debug)]
pub enum CreateRelayerError {
    #[error("Relayer could not be saved in DB - name: {0}, chainId: {1}: {0}")]
    CouldNotSaveRelayerDb(String, ChainId, PostgresError),

    #[error("Relayer could not update DB - name: {0}, chainId: {1}: {2}")]
    CouldNotUpdateRelayerInfoDb(String, ChainId, PostgresError),

    #[error("Relayer did not return init information - name: {0}, chainId: {1}")]
    NoSaveRelayerInitInfoReturnedDb(String, ChainId),

    #[error("Wallet error - name: {0}, chainId: {1}: {0}")]
    WalletError(String, ChainId, Box<dyn Error + Send + Sync>),

    #[error("Relayer {0} not found for cloning")]
    RelayerNotFound(RelayerId),
}

impl PostgresClient {
    pub async fn create_relayer(
        &self,
        name: &str,
        chain_id: &ChainId,
        evm_provider: &EvmProvider,
        clone_relayer_id: Option<RelayerId>,
    ) -> Result<Relayer, CreateRelayerError> {
        let relayer_id = RelayerId::new();

        let wallet_index = if let Some(clone_id) = clone_relayer_id {
            let source_relayer = self
                .get_relayer(&clone_id)
                .await
                .map_err(|e| {
                    CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
                })?
                .ok_or_else(|| CreateRelayerError::RelayerNotFound(clone_id.clone()))?;

            self.execute(
                "INSERT INTO relayer.record (id, name, chain_id, wallet_index, max_gas_price_cap, paused, allowlisted_addresses_only, eip_1559_enabled)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &relayer_id,
                    &name,
                    chain_id,
                    &(source_relayer.wallet_index as i32),
                    &source_relayer.max_gas_price,
                    &source_relayer.paused,
                    &source_relayer.allowlisted_only,
                    &source_relayer.eip_1559_enabled,
                ],
            )
                .await
                .map_err(|e| {
                    println!("{}", e.to_string());
                    CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
                })?;

            source_relayer.wallet_index as i32
        } else {
            let query = "
                WITH new_wallet_index AS (
                    SELECT COALESCE(MAX(wallet_index), -1) + 1 AS wallet_index
                    FROM relayer.record
                    WHERE chain_id = $3
                )
                INSERT INTO relayer.record (id, name, chain_id, wallet_index)
                SELECT $1, $2, $3, wallet_index
                FROM new_wallet_index
                RETURNING wallet_index;
            ";

            let rows = self.query(query, &[&relayer_id, &name, &chain_id]).await.map_err(|e| {
                CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
            })?;

            if let Some(row) = rows.first() {
                row.get("wallet_index")
            } else {
                return Err(CreateRelayerError::NoSaveRelayerInitInfoReturnedDb(
                    name.to_string(),
                    *chain_id,
                ));
            }
        };

        let address = evm_provider.create_wallet(wallet_index as u32).await.map_err(|e| {
            CreateRelayerError::WalletError(name.to_string(), *chain_id, Box::new(e))
        })?;

        self.execute(
            "UPDATE relayer.record SET address = $1 WHERE chain_id = $2 AND wallet_index = $3",
            &[&address, chain_id, &wallet_index],
        )
        .await
        .map_err(|e| {
            CreateRelayerError::CouldNotUpdateRelayerInfoDb(name.to_string(), *chain_id, e)
        })?;

        let relayer = self.get_relayer(&relayer_id).await.map_err(|e| {
            CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
        })?;

        match relayer {
            Some(relayer) => Ok(relayer),
            None => Err(CreateRelayerError::NoSaveRelayerInitInfoReturnedDb(
                name.to_string(),
                *chain_id,
            )),
        }
    }

    pub async fn delete_relayer(&self, relayer_id: &RelayerId) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET deleted = TRUE
                WHERE id = $1
                ",
                &[relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn pause_relayer(&self, relayer_id: &RelayerId) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET paused = TRUE
                WHERE id = $1
                ",
                &[relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn unpause_relayer(&self, relayer_id: &RelayerId) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET paused = FALSE
                WHERE id = $1
                ",
                &[relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn create_relayer_api_key(
        &self,
        relayer_id: &RelayerId,
        api_key: &str,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                INSERT INTO relayer.api_key(api_key, relayer_id)
                VALUES ($1, $2)
                ",
                &[&api_key, relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn delete_relayer_api_key(
        &self,
        relayer_id: &RelayerId,
        api_key: &str,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.api_key
                SET deleted = TRUE
                WHERE api_key = $1
                AND relayer_id = $2
                ",
                &[&api_key, relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn update_relayer_max_gas_price(
        &self,
        relayer_id: &RelayerId,
        cap: Option<GasPrice>,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET max_gas_price_cap = $1
                WHERE id = $2
                ",
                &[&cap, relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn update_relayer_eip_1559_status(
        &self,
        relayer_id: &RelayerId,
        enable: &bool,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET eip_1559_enabled = $1
                AND id = $2
                ",
                &[enable, relayer_id],
            )
            .await?;

        Ok(())
    }

    pub async fn relayer_add_allowlist_address(
        &self,
        relayer_id: &RelayerId,
        address: &EvmAddress,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                INSERT INTO relayer.allowlisted_address(address, relayer_id)
                VALUES ($1, $2)
                ON CONFLICT DO NOTHING;
                ",
                &[&address, relayer_id],
            )
            .await?;

        self.relayer_allowlist_addresses_only_sync_state(relayer_id).await?;

        Ok(())
    }

    pub async fn relayer_delete_allowlist_address(
        &self,
        relayer_id: &RelayerId,
        address: &EvmAddress,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                DELETE FROM relayer.allowlisted_address
                WHERE address = $1
                AND relayer_id = $2;
                ",
                &[&address, relayer_id],
            )
            .await?;

        self.relayer_allowlist_addresses_only_sync_state(relayer_id).await?;

        Ok(())
    }

    async fn relayer_allowlist_addresses_only_sync_state(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<(), PostgresError> {
        let _ = self
            .execute(
                "
                UPDATE relayer.record
                SET allowlisted_addresses_only = COALESCE(
                    (
                        SELECT TRUE 
                        FROM relayer.api_key 
                        WHERE relayer_id = $1 
                        AND deleted = FALSE 
                        LIMIT 1
                    ), 
                    FALSE)
                WHERE id = $1
                ",
                &[relayer_id],
            )
            .await?;

        Ok(())
    }
}
