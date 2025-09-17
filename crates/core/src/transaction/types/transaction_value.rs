use std::{error::Error, str::FromStr};
use std::fmt::Display;
use alloy::primitives::U256;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionValue(U256);

impl TransactionValue {
    pub fn zero() -> TransactionValue {
        TransactionValue(U256::from(0))
    }

    pub fn into_inner(self) -> U256 {
        self.0
    }

    pub fn new(value: U256) -> Self {
        TransactionValue(value)
    }
}

impl Default for TransactionValue {
    fn default() -> Self {
        TransactionValue::zero()
    }
}

impl PartialEq for TransactionValue {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Display for TransactionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> FromSql<'a> for TransactionValue {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let decimal = rust_decimal::Decimal::from_sql(ty, raw)?;
        let value_str = decimal.to_string();

        let u256_value = U256::from_str(&value_str)
            .map_err(|e| format!("Failed to convert decimal to U256: {}", e))?;
        Ok(TransactionValue(u256_value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

impl ToSql for TransactionValue {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match rust_decimal::Decimal::from_str(&self.0.to_string()) {
            Ok(decimal) => decimal.to_sql(ty, out),
            Err(e) => Err(format!("Failed to convert to decimal: {}", e).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }

    tokio_postgres::types::to_sql_checked!();
}

impl FromStr for TransactionValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        U256::from_str(s).map(TransactionValue).map_err(|e| e.to_string())
    }
}

impl From<TransactionValue> for U256 {
    fn from(value: TransactionValue) -> Self {
        value.0
    }
}

impl From<U256> for TransactionValue {
    fn from(value: U256) -> Self {
        TransactionValue(value)
    }
}
