use std::{error::Error, str::FromStr};

use alloy::primitives::U256;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

use crate::shared::{from_sql_u256, to_sql_u256};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionValue(U256);

impl TransactionValue {
    pub fn zero() -> TransactionValue {
        TransactionValue(U256::from(0))
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

impl<'a> FromSql<'a> for TransactionValue {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        from_sql_u256(ty, raw).map(TransactionValue)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for TransactionValue {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        to_sql_u256(self.0, ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
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
