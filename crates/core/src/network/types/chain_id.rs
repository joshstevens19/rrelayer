use std::{
    error::Error,
    fmt::Display,
    hash::{Hash, Hasher},
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Deserializer, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Eq)]
pub struct ChainId(u64);

impl Default for ChainId {
    /// Returns the default ChainId (Ethereum mainnet, chain ID 1).
    ///
    /// # Returns
    /// * `ChainId` - Default chain ID set to 1 (Ethereum mainnet)
    fn default() -> Self {
        ChainId(1)
    }
}

impl ChainId {
    /// Creates a new ChainId from a u64 value.
    ///
    /// # Arguments
    /// * `id` - The numeric chain identifier
    ///
    /// # Returns
    /// * `ChainId` - New chain ID instance
    pub fn new(id: u64) -> Self {
        ChainId(id)
    }

    /// Extracts the underlying u64 value from the ChainId.
    ///
    /// # Returns
    /// * `u64` - The numeric chain identifier
    pub fn u64(&self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ChainId {
    /// Deserializes a ChainId from a u64 value.
    ///
    /// # Arguments
    /// * `deserializer` - The deserializer to read from
    ///
    /// # Returns
    /// * `Ok(ChainId)` - Successfully deserialized chain ID
    /// * `Err(D::Error)` - If deserialization fails
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = u64::deserialize(deserializer)?;

        Ok(ChainId(id))
    }
}

impl Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for ChainId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for ChainId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for ChainId {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        i64::from_sql(ty, raw).map(|value| ChainId::from(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

impl ToSql for ChainId {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        i64::to_sql(&self.clone().into(), ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseChainIdError;

impl Display for ParseChainIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid chain id")
    }
}

impl Error for ParseChainIdError {}

impl FromStr for ChainId {
    type Err = ParseChainIdError;

    /// Parses a ChainId from a string representation.
    ///
    /// # Arguments
    /// * `param` - String containing the numeric chain ID
    ///
    /// # Returns
    /// * `Ok(ChainId)` - Successfully parsed chain ID
    /// * `Err(ParseChainIdError)` - If string cannot be parsed as valid u64
    fn from_str(param: &str) -> Result<Self, Self::Err> {
        u64::from_str(param).map(ChainId).map_err(|_| ParseChainIdError)
    }
}

impl From<ChainId> for u64 {
    fn from(chain_id: ChainId) -> Self {
        chain_id.0
    }
}

impl From<u64> for ChainId {
    fn from(chain_id: u64) -> Self {
        ChainId(chain_id)
    }
}

impl From<i64> for ChainId {
    fn from(chain_id: i64) -> Self {
        ChainId(chain_id as u64)
    }
}

impl From<ChainId> for i64 {
    fn from(chain_id: ChainId) -> Self {
        chain_id.0 as i64
    }
}
