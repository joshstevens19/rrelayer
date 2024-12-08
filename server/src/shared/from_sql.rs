use std::{error::Error, str::FromStr};

use alloy::primitives::U256;
use tokio_postgres::types::Type;

pub fn from_sql_u256(ty: &Type, raw: &[u8]) -> Result<U256, Box<dyn Error + Sync + Send>> {
    if *ty == Type::INT4 {
        let int_value =
            i32::from_be_bytes(raw.try_into().map_err(|_| "Failed to convert bytes to i32")?);

        Ok(U256::from(int_value as u32))
    } else if *ty == Type::VARCHAR {
        let string_value =
            std::str::from_utf8(raw).map_err(|_| "Failed to convert bytes to string")?;

        Ok(U256::from_str(string_value)?)
    } else {
        Err("Expected type INT4 or VARCHAR for U256".into())
    }
}
