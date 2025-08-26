use std::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
};

use alloy::primitives::{B256, U256};
use bytes::BytesMut;
use fmt::Display;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

use crate::shared::from_param_u256;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct BlockHash(B256);

impl BlockHash {
    /// Creates a new BlockHash wrapper around a B256.
    ///
    /// # Arguments
    /// * `block_hash` - The B256 block hash to wrap
    ///
    /// # Returns
    /// * `Self` - A new BlockHash instance
    pub fn new(block_hash: B256) -> Self {
        BlockHash(block_hash)
    }
}

impl Hash for BlockHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for BlockHash {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for BlockHash {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let block_hash = B256::from_slice(raw);

        Ok(BlockHash(block_hash))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }
}

impl ToSql for BlockHash {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let value_as_string = self.0.to_string();

        out.extend_from_slice(value_as_string.as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseBlockHashError(String);

impl Display for ParseBlockHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid block hash: {}", self.0)
    }
}

impl Error for ParseBlockHashError {}

impl FromStr for BlockHash {
    type Err = ParseBlockHashError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        from_param_u256(param)
            .map(|hash| BlockHash(hash.into()))
            .map_err(|e| ParseBlockHashError(e.to_string()))
    }
}

impl From<BlockHash> for B256 {
    fn from(block_hash: BlockHash) -> Self {
        block_hash.0
    }
}

impl From<BlockHash> for U256 {
    fn from(block_hash: BlockHash) -> Self {
        block_hash.0.into()
    }
}

impl From<U256> for BlockHash {
    fn from(block_hash: U256) -> Self {
        BlockHash(B256::from(block_hash))
    }
}
