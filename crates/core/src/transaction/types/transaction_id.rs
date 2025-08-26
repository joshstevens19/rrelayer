use std::{
    error::Error,
    fmt::Display,
    hash::{Hash, Hasher},
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};
use uuid::Uuid;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionId(Uuid);

impl Hash for TransactionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TransactionId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TransactionId {
    /// Creates a new random transaction ID.
    ///
    /// # Returns
    /// * `TransactionId` - A new unique transaction identifier
    pub fn new() -> TransactionId {
        TransactionId(Uuid::new_v4())
    }
}

impl<'a> FromSql<'a> for TransactionId {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let uuid = Uuid::from_sql(ty, raw)?;

        Ok(TransactionId(uuid))
    }

    fn from_sql_null(ty: &Type) -> Result<Self, Box<dyn Error + Sync + Send + 'static>> {
        let uuid = Uuid::from_sql_null(ty)?;
        Ok(TransactionId(uuid))
    }

    fn from_sql_nullable(
        ty: &Type,
        raw: Option<&'a [u8]>,
    ) -> Result<Self, Box<dyn Error + Sync + Send + 'static>> {
        match raw {
            Some(raw) => Self::from_sql(ty, raw),
            None => Self::from_sql_null(ty),
        }
    }

    fn accepts(ty: &Type) -> bool {
        <Uuid as FromSql>::accepts(ty)
    }
}

impl ToSql for TransactionId {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send + 'static>> {
        // Delegate to the Uuid's ToSql implementation
        self.0.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <Uuid as FromSql>::accepts(ty)
    }

    to_sql_checked!();
}

impl FromStr for TransactionId {
    type Err = String;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(param).map(TransactionId).map_err(|e| e.to_string())
    }
}

impl From<Uuid> for TransactionId {
    fn from(uuid: Uuid) -> Self {
        TransactionId(uuid)
    }
}
