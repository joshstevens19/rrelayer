use std::{
    error::Error,
    hash::{Hash, Hasher},
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct BlockNumber(u64);

impl BlockNumber {
    pub fn new(number: u64) -> Self {
        BlockNumber(number)
    }
}

impl Hash for BlockNumber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for BlockNumber {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for BlockNumber {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        i64::from_sql(ty, raw).map(BlockNumber::from)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

impl ToSql for BlockNumber {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        i64::to_sql(&(*self).into(), ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseBlockNumberError(String);

impl std::fmt::Display for ParseBlockNumberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid block number: {}", self.0)
    }
}

impl Error for ParseBlockNumberError {}

impl FromStr for BlockNumber {
    type Err = ParseBlockNumberError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        u64::from_str(param).map(BlockNumber).map_err(|e| ParseBlockNumberError(e.to_string()))
    }
}

impl From<BlockNumber> for u64 {
    fn from(block_number: BlockNumber) -> Self {
        block_number.0
    }
}

impl From<u64> for BlockNumber {
    fn from(block_number: u64) -> Self {
        BlockNumber(block_number)
    }
}

impl From<BlockNumber> for i64 {
    fn from(block_number: BlockNumber) -> Self {
        block_number.0 as i64
    }
}

impl From<i64> for BlockNumber {
    fn from(block_number: i64) -> Self {
        BlockNumber(block_number as u64)
    }
}
