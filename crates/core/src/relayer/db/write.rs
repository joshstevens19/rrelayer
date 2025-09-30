use std::error::Error;
use thiserror::Error;

use crate::shared::{internal_server_error, not_found, HttpError};
use crate::{
    gas::GasPrice,
    network::ChainId,
    postgres::{PostgresClient, PostgresError},
    provider::EvmProvider,
    relayer::types::{Relayer, RelayerId},
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

impl From<CreateRelayerError> for HttpError {
    fn from(value: CreateRelayerError) -> Self {
        if matches!(value, CreateRelayerError::RelayerNotFound(_)) {
            return not_found("Could not find relayer".to_string());
        }

        internal_server_error(Some(value.to_string()))
    }
}

impl From<PostgresError> for CreateRelayerError {
    fn from(value: PostgresError) -> Self {
        CreateRelayerError::CouldNotSaveRelayerDb("Unknown".to_string(), ChainId::new(0), value)
    }
}

pub enum CreateRelayerMode {
    Clone(RelayerId),
    Create,
}

impl PostgresClient {
    pub async fn create_relayer(
        &self,
        name: &str,
        chain_id: &ChainId,
        evm_provider: &EvmProvider,
        mode: CreateRelayerMode,
    ) -> Result<Relayer, CreateRelayerError> {
        let new_relayer_id = RelayerId::new();

        match &mode {
            CreateRelayerMode::Clone(clone_relayer_id) => {
                let source_relayer = self
                    .get_relayer(&clone_relayer_id)
                    .await
                    .map_err(|e| {
                        CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
                    })?
                    .ok_or_else(|| CreateRelayerError::RelayerNotFound(*clone_relayer_id))?;

                let wallet_index = source_relayer.wallet_index as i32;
                let address =
                    evm_provider.create_wallet(wallet_index as u32).await.map_err(|e| {
                        CreateRelayerError::WalletError(name.to_string(), *chain_id, Box::new(e))
                    })?;

                self.execute(
                    "INSERT INTO relayer.record (id, name, chain_id, wallet_index, max_gas_price_cap, paused, eip_1559_enabled, address)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    &[
                        &new_relayer_id,
                        &name,
                        chain_id,
                        &wallet_index,
                        &source_relayer.max_gas_price,
                        &source_relayer.paused,
                        &source_relayer.eip_1559_enabled,
                        &address,
                    ],
                )
                .await
                .map_err(|e| {
                    CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e)
                })?;
            }
            CreateRelayerMode::Create => {
                let evm_provider_clone = evm_provider.clone();
                let new_relayer_id_val = new_relayer_id;
                let name_val = name.to_string();
                let chain_id_val = *chain_id;

                self.with_transaction(move |tx| {
                    Box::pin(async move {
                        let query = "
                            WITH new_wallet_index AS (
                                SELECT COALESCE(MAX(wallet_index), -1) + 1 AS wallet_index
                                FROM relayer.record
                                WHERE chain_id = $3
                            )
                            INSERT INTO relayer.record (id, name, chain_id, wallet_index)
                            SELECT $1, $2, $3, wallet_index
                            FROM new_wallet_index
                            RETURNING wallet_index";

                        let rows = tx.query(query, &[&new_relayer_id_val, &name_val, &chain_id_val]).await.map_err(PostgresError::PgError)?;

                        let wallet_index: i32 = rows.first()
                            .map(|row| row.get("wallet_index"))
                            .unwrap_or_else(|| panic!("No wallet index returned"));

                        let address = evm_provider_clone.create_wallet(wallet_index as u32).await
                            .unwrap_or_else(|e| panic!("Wallet creation failed: {}", e));

                        tx.execute(
                            "UPDATE relayer.record SET address = $1 WHERE chain_id = $2 AND wallet_index = $3",
                            &[&address, &chain_id_val, &wallet_index],
                        )
                        .await.map_err(PostgresError::PgError)?;

                        Ok(())
                    })
                })
                .await
                .map_err(|e| CreateRelayerError::CouldNotSaveRelayerDb(name.to_string(), *chain_id, e))?
            }
        };

        let relayer = self.get_relayer(&new_relayer_id).await.map_err(|e| {
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
                WHERE id = $2
                ",
                &[enable, relayer_id],
            )
            .await?;

        Ok(())
    }
}
