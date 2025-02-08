use tokio_postgres::Row;

use crate::user::types::User;

pub fn build_user(row: &Row) -> User {
    User { address: row.get("address"), role: row.get("role") }
}
