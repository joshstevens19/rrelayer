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

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionHash(pub TxHash);

impl TransactionHash {
    pub fn hex(&self) -> String {
        format!("{:?}", self.0)
    }

    pub fn from_alloy_hash(hash: &TxHash) -> Self {
        Self(hash.clone())
    }
}

impl Display for TransactionHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", self.0)
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
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if *ty == Type::BPCHAR {
            let hash = String::from_utf8(raw.to_vec())?;

            Ok(TransactionHash(TxHash::from_str(&hash)?))
        } else {
            Err("Expected type BPCHAR for TransactionHash".into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BPCHAR
    }
}

impl ToSql for TransactionHash {
    fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let hash = self.0.to_string();

        out.extend_from_slice(hash.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BPCHAR
    }

    tokio_postgres::types::to_sql_checked!();
}

impl FromStr for TransactionHash {
    type Err = String;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        TxHash::from_str(param).map(TransactionHash).map_err(|e| e.to_string())
    }
}
