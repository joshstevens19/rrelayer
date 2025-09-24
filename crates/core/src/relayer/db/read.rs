use super::builders::build_relayer;
use crate::{
    network::ChainId,
    postgres::{PostgresClient, PostgresError},
    relayer::types::{Relayer, RelayerId},
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
};

impl PostgresClient {
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

    pub async fn get_relayer_by_address(
        &self,
        address: &EvmAddress,
        chain_id: &ChainId,
    ) -> Result<Option<Relayer>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer.record
                    WHERE address = $1
                    AND chain_id = $2
                    AND deleted = FALSE;
                ",
                &[address, chain_id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_relayer(&row))),
        }
    }
}
