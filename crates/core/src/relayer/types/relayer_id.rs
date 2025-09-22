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
pub struct RelayerId(Uuid);

impl RelayerId {
    pub fn new() -> RelayerId {
        RelayerId(Uuid::new_v4())
    }
}

impl Display for RelayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for RelayerId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for RelayerId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for RelayerId {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let uuid = Uuid::from_sql(ty, raw)?;

        Ok(RelayerId(uuid))
    }

    fn from_sql_null(ty: &Type) -> Result<Self, Box<dyn Error + Sync + Send + 'static>> {
        let uuid = Uuid::from_sql_null(ty)?;
        Ok(RelayerId(uuid))
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

impl ToSql for RelayerId {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send + 'static>> {
        self.0.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <Uuid as FromSql>::accepts(ty)
    }

    to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseRelayerIdError(String);

impl Display for ParseRelayerIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid relayer id: {}", self.0)
    }
}

impl Error for ParseRelayerIdError {}

impl FromStr for RelayerId {
    type Err = ParseRelayerIdError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(param).map(RelayerId).map_err(|e| ParseRelayerIdError(e.to_string()))
    }
}

impl From<RelayerId> for Uuid {
    fn from(relayer_id: RelayerId) -> Self {
        relayer_id.0
    }
}

impl From<Uuid> for RelayerId {
    fn from(relayer_id: Uuid) -> Self {
        RelayerId(relayer_id)
    }
}
