use super::builders::build_user;
use crate::{
    postgres::{PostgresClient, PostgresError},
    shared::common_types::{EvmAddress, PagingContext, PagingResult},
    user::types::User,
};

impl PostgresClient {
    pub async fn get_user(&self, address: &EvmAddress) -> Result<Option<User>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM authentication.user_access
                    WHERE address = $1;
                ",
                &[address],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_user(&row))),
        }
    }

    pub async fn get_users(
        &self,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<User>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM authentication.user_access
                    LIMIT $1
                    OFFSET $2;
                ",
                &[&(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<User> = rows.iter().map(build_user).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }
}
