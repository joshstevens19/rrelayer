use std::{
    error::Error,
    fmt::Display,
    hash::{Hash, Hasher},
    str::FromStr,
};

use alloy::primitives::TxHash;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

use crate::postgres::PgType;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionHash(TxHash);

impl TransactionHash {
    pub fn hex(&self) -> String {
        format!("{:?}", self.0)
    }

    pub fn from_alloy_hash(hash: &TxHash) -> Self {
        Self(*hash)
    }

    pub fn into_alloy_hash(self) -> TxHash {
        self.0
    }
}

impl Display for TransactionHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for TransactionHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TransactionHash {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for TransactionHash {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if raw.len() != 32 {
            return Err("Invalid byte length for transaction hash".into());
        }

        let tx_hash = TxHash::from_slice(raw);

        Ok(TransactionHash(tx_hash))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }
}

impl ToSql for TransactionHash {
    fn to_sql(
        &self,
        _ty: &PgType,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(self.0.as_slice());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    tokio_postgres::types::to_sql_checked!();
}

impl FromStr for TransactionHash {
    type Err = String;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        TxHash::from_str(param).map(TransactionHash).map_err(|e| e.to_string())
    }
}
