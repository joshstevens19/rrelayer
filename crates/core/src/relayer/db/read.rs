use super::builders::build_relayer;
use crate::{
    network::types::ChainId,
    postgres::{PostgresClient, PostgresError},
    relayer::types::{Relayer, RelayerId},
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
};

impl PostgresClient {
    /// Retrieves a paginated list of all relayers from the database.
    ///
    /// This method queries the database for all relayer records with pagination support.
    /// Results are limited by the paging context and returned as a paginated result.
    ///
    /// # Arguments
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<Relayer>)` - Paginated list of relayers
    /// * `Err(PostgresError)` - If database query fails
    pub async fn get_relayers(
        &self,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Relayer>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.record
                    LIMIT $1
                    OFFSET $2;
                ",
                &[&(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Relayer> = rows.iter().map(build_relayer).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    /// Retrieves a paginated list of relayers for a specific blockchain network.
    ///
    /// This method queries the database for relayer records filtered by chain ID,
    /// excluding soft-deleted relayers. Results are paginated according to the paging context.
    ///
    /// # Arguments
    /// * `chain_id` - The blockchain network ID to filter relayers by
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<Relayer>)` - Paginated list of relayers for the specified chain
    /// * `Err(PostgresError)` - If database query fails
    pub async fn get_relayers_for_chain(
        &self,
        chain_id: &ChainId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Relayer>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.record
                    WHERE chain_id = $1
                    AND deleted = FALSE
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&chain_id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Relayer> = rows.iter().map(build_relayer).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_all_relayers_for_chain(
        &self,
        chain_id: &ChainId,
    ) -> Result<Vec<Relayer>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.record
                    WHERE chain_id = $1
                    AND deleted = FALSE
                ",
                &[&chain_id],
            )
            .await?;

        let results: Vec<Relayer> = rows.iter().map(build_relayer).collect();

        Ok(results)
    }

    /// Retrieves a single relayer by its unique identifier.
    ///
    /// This method queries the database for a specific relayer using its ID,
    /// excluding soft-deleted relayers. Returns None if the relayer doesn't exist.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer to retrieve
    ///
    /// # Returns
    /// * `Ok(Some(Relayer))` - If the relayer is found and not deleted
    /// * `Ok(None)` - If the relayer doesn't exist or is soft-deleted
    /// * `Err(PostgresError)` - If database query fails
    pub async fn get_relayer(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<Option<Relayer>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer.record
                    WHERE id = $1
                    AND deleted = FALSE;
                ",
                &[relayer_id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_relayer(&row))),
        }
    }

    /// Retrieves a paginated list of allowlisted addresses for a specific relayer.
    ///
    /// This method queries the database for all Ethereum addresses that are allowed
    /// to use the specified relayer for transaction processing, ordered by creation date.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<EvmAddress>)` - Paginated list of allowlisted Ethereum addresses
    /// * `Err(PostgresError)` - If database query fails
    pub async fn relayer_get_allowlist_addresses(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<EvmAddress>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT 
                        r.address
                    FROM relayer.allowlisted_address r
                    WHERE r.relayer_id = $1
                    ORDER BY r.created_at DESC
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&relayer_id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<EvmAddress> = rows.iter().map(|row| row.get("address")).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    /// Checks if an Ethereum address is allowlisted for a specific relayer.
    ///
    /// This method verifies whether the given Ethereum address is present in the
    /// allowlist for the specified relayer, allowing it to use the relayer's services.
    ///
    /// # Arguments
    /// * `relayer_id` - The unique identifier of the relayer
    /// * `address` - The Ethereum address to check
    ///
    /// # Returns
    /// * `Ok(true)` - If the address is allowlisted for the relayer
    /// * `Ok(false)` - If the address is not allowlisted
    /// * `Err(PostgresError)` - If database query fails
    pub async fn is_relayer_allowlist_address(
        &self,
        relayer_id: &RelayerId,
        address: &EvmAddress,
    ) -> Result<bool, PostgresError> {
        let rows = self
            .query_one_or_none(
                "
                    SELECT 1
                    FROM relayer.allowlisted_address r
                    WHERE r.relayer_id = $1
                    AND r.address = $2;
                ",
                &[relayer_id, address],
            )
            .await?;

        if rows.is_none() {
            return Ok(false);
        }

        Ok(true)
    }
}
