pub(super) fn next_normal_wallet_index_sql(chain_id_parameter: &str) -> String {
    format!(
        "
            SELECT COALESCE(MAX(wallet_index), -1) + 1 AS wallet_index
            FROM relayer.record
            WHERE chain_id = {chain_id_parameter}
              AND is_private_key = FALSE
              AND deleted = FALSE
              AND wallet_index >= 0
        "
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_normal_wallet_index_from_rows(rows: &[(i32, bool, bool)]) -> i32 {
        rows.iter()
            .filter(|(wallet_index, is_private_key, deleted)| {
                *wallet_index >= 0 && !*is_private_key && !*deleted
            })
            .map(|(wallet_index, _, _)| *wallet_index)
            .max()
            .unwrap_or(-1)
            + 1
    }

    #[test]
    fn private_key_rows_do_not_consume_normal_wallet_indexes() {
        assert_eq!(next_normal_wallet_index_from_rows(&[(-1, true, false), (-2, true, false)]), 0);
    }

    #[test]
    fn deleted_rows_may_be_safely_reused_without_colliding_with_live_rows() {
        let rows = [(-1, true, false), (4, false, true), (2, false, false)];
        assert_eq!(next_normal_wallet_index_from_rows(&rows), 3);
    }

    #[test]
    fn allocation_sql_filters_to_live_normal_namespace() {
        let sql = next_normal_wallet_index_sql("$3");
        assert!(sql.contains("chain_id = $3"));
        assert!(sql.contains("is_private_key = FALSE"));
        assert!(sql.contains("deleted = FALSE"));
        assert!(sql.contains("wallet_index >= 0"));
    }
}
