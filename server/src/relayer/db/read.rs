use super::builders::build_relayer_from_relayer_view;
use crate::{
    network::types::ChainId,
    postgres::PostgresClient,
    relayer::types::{Relayer, RelayerId},
    shared::common_types::{ApiKey, EvmAddress, PagingContext, PagingResult},
};

impl PostgresClient {
    pub async fn get_relayers(
        &self,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Relayer>, tokio_postgres::Error> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer_view
                    LIMIT $1
                    OFFSET $2;
                ",
                &[&(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Relayer> = rows.iter().map(build_relayer_from_relayer_view).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_relayers_for_chain(
        &self,
        chain_id: &ChainId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Relayer>, tokio_postgres::Error> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer_view
                    WHERE chain_id = $1
                    AND deleted = FALSE
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&chain_id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Relayer> = rows.iter().map(build_relayer_from_relayer_view).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_relayer(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<Option<Relayer>, tokio_postgres::Error> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer_view
                    WHERE id = $1
                    AND deleted = FALSE;
                ",
                &[relayer_id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_relayer_from_relayer_view(&row))),
        }
    }

    pub async fn is_relayer_api_key(
        &self,
        relayer_id: &RelayerId,
        api_key: &ApiKey,
    ) -> Result<bool, tokio_postgres::Error> {
        let rows = self
            .query_one_or_none(
                "
                    SELECT 1
                    FROM relayer_api_key r
                    WHERE r.relayer_id = $1
                    AND r.deleted = FALSE
                    AND r.api_key = $2;
                ",
                &[relayer_id, api_key],
            )
            .await?;

        if rows.is_none() {
            return Ok(false);
        }

        Ok(true)
    }

    pub async fn get_relayer_api_keys(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<String>, tokio_postgres::Error> {
        let rows = self
            .query(
                "
                    SELECT 
                        r.api_key
                    FROM relayer_api_key r
                    WHERE r.relayer_id = $1
                    AND r.deleted = FALSE
                    ORDER BY r.created_at DESC
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&relayer_id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<String> = rows.iter().map(|row| row.get("api_key")).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn relayer_get_allowlist_addresses(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<String>, tokio_postgres::Error> {
        let rows = self
            .query(
                "
                    SELECT 
                        r.address
                    FROM relayer_allowlisted_address r
                    WHERE r.relayer_id = $1
                    ORDER BY r.created_at DESC
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&relayer_id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<String> = rows.iter().map(|row| row.get("address")).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn is_relayer_allowlist_address(
        &self,
        relayer_id: &RelayerId,
        address: &EvmAddress,
    ) -> Result<bool, tokio_postgres::Error> {
        let rows = self
            .query_one_or_none(
                "
                    SELECT 1
                    FROM relayer_allowlisted_address r
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
