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
    /// Creates a new relayer in the database and initializes its wallet.
    ///
    /// This method creates a new relayer entry in the database, assigns it a wallet index,
    /// creates the corresponding wallet using the EVM provider, and updates the database
    /// with the wallet address. Optionally clones configuration from an existing relayer.
    ///
    /// # Arguments
    /// * `name` - The name for the new relayer
    /// * `chain_id` - The blockchain network ID for the relayer
    /// * `evm_provider` - The EVM provider for wallet operations
    /// * `clone_relayer_id` - Optional ID of existing relayer to clone settings from
    ///
    /// # Returns
    /// * `Ok(Relayer)` - The newly created relayer with all details
    /// * `Err(CreateRelayerError)` - If creation fails at any step
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

    /// Soft deletes a relayer by marking it as deleted in the database.
    ///
    /// This method performs a soft delete by setting the deleted flag to true rather
    /// than physically removing the relayer record. This preserves audit trails and
    /// allows for potential recovery.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer to delete
    ///
    /// # Returns
    /// * `Ok(())` - If the relayer was successfully marked as deleted
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Pauses a relayer by updating its status in the database.
    ///
    /// This method sets the paused flag to true for the specified relayer,
    /// which prevents it from processing new transactions.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer to pause
    ///
    /// # Returns
    /// * `Ok(())` - If the relayer was successfully paused
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Unpauses a relayer by updating its status in the database.
    ///
    /// This method sets the paused flag to false for the specified relayer,
    /// allowing it to resume processing transactions.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer to unpause
    ///
    /// # Returns
    /// * `Ok(())` - If the relayer was successfully unpaused
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Creates a new API key for a specific relayer.
    ///
    /// This method inserts a new API key record in the database, associating it
    /// with the specified relayer. The API key can be used for authenticated access
    /// to relayer-specific operations.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `api_key` - The API key string to store
    ///
    /// # Returns
    /// * `Ok(())` - If the API key was successfully created
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Soft deletes an API key for a specific relayer.
    ///
    /// This method marks an API key as deleted rather than physically removing it,
    /// preserving audit trails while revoking access for that key.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `api_key` - The API key string to delete
    ///
    /// # Returns
    /// * `Ok(())` - If the API key was successfully marked as deleted
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Updates the maximum gas price cap for a relayer.
    ///
    /// This method sets or removes the gas price limit for a relayer. When set,
    /// the relayer will refuse to process transactions that would require gas prices
    /// above this limit. Setting to None removes the cap.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `cap` - The new gas price cap (None to remove the cap)
    ///
    /// # Returns
    /// * `Ok(())` - If the gas price cap was successfully updated
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Updates the EIP-1559 transaction support status for a relayer.
    ///
    /// This method enables or disables EIP-1559 (London hard fork) transaction support
    /// for a relayer. When enabled, the relayer uses type-2 transactions with base fee
    /// and priority fee. When disabled, it uses legacy transactions with gas price.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `enable` - Whether to enable EIP-1559 transactions (true) or use legacy (false)
    ///
    /// # Returns
    /// * `Ok(())` - If the EIP-1559 status was successfully updated
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Adds an Ethereum address to a relayer's allowlist.
    ///
    /// This method inserts an address into the allowlist table for the specified relayer.
    /// If the address is already allowlisted, the operation is ignored (ON CONFLICT DO NOTHING).
    /// After adding the address, it syncs the allowlist-only state for the relayer.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `address` - The Ethereum address to add to the allowlist
    ///
    /// # Returns
    /// * `Ok(())` - If the address was successfully added or already existed
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Removes an Ethereum address from a relayer's allowlist.
    ///
    /// This method deletes an address from the allowlist table for the specified relayer.
    /// After removing the address, it syncs the allowlist-only state for the relayer.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `address` - The Ethereum address to remove from the allowlist
    ///
    /// # Returns
    /// * `Ok(())` - If the address was successfully removed
    /// * `Err(PostgresError)` - If the database operation fails
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

    /// Synchronizes the allowlist-only state for a relayer based on existing allowlist entries.
    ///
    /// This private method updates the allowlisted_addresses_only flag for a relayer based on
    /// whether any allowlist addresses exist. The query incorrectly checks for API keys instead
    /// of allowlisted addresses - this appears to be a bug in the original code.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    ///
    /// # Returns
    /// * `Ok(())` - If the state was successfully synchronized
    /// * `Err(PostgresError)` - If the database operation fails
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
                        FROM relayer.allowlisted_address 
                        WHERE relayer_id = $1 
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
