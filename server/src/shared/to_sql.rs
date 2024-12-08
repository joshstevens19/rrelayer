use std::error::Error;

use alloy::primitives::U256;
use bytes::BytesMut;
use tokio_postgres::types::{IsNull, Type};

pub fn to_sql_u256(
    value: U256,
    ty: &Type,
    out: &mut BytesMut,
) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
    if *ty == Type::VARCHAR {
        let value_as_string = value.to_string();

        out.extend_from_slice(value_as_string.as_bytes());
        Ok(IsNull::No)
    } else {
        Err("Expected VARCHAR type for U256".into())
    }
}
