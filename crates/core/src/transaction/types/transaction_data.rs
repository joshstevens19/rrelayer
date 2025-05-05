use std::{
    error::Error,
    hash::{Hash, Hasher},
    str::FromStr,
};

use alloy::{hex, primitives::Bytes};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionData(Bytes);

impl TransactionData {
    pub fn to_formatted_hex(&self) -> String {
        format!("{:?}", self.0)
    }

    pub fn new(data: Bytes) -> Self {
        Self(data)
    }

    pub fn empty() -> Self {
        Self(Bytes::new())
    }

    pub fn into_inner(self) -> Bytes {
        self.0
    }

    pub fn hex(&self) -> String {
        hex::encode(&self.0)
    }
}

impl Default for TransactionData {
    fn default() -> Self {
        TransactionData::empty()
    }
}

impl Hash for TransactionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TransactionData {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl From<TransactionData> for Bytes {
    fn from(data: TransactionData) -> Self {
        data.0
    }
}

impl<'a> FromSql<'a> for TransactionData {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if <Self as FromSql>::accepts(ty) {
            Ok(TransactionData(Bytes::from(raw.to_vec())))
        } else {
            Err("Unsupported type".into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for TransactionData {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if <Self as ToSql>::accepts(ty) {
            out.extend_from_slice(&self.0);
            Ok(IsNull::No)
        } else {
            Err("Unsupported type".into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    tokio_postgres::types::to_sql_checked!();
}

impl FromStr for TransactionData {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        hex::decode(s).map(Bytes::from).map(TransactionData).map_err(|e| e.to_string())
    }
}

impl From<Bytes> for TransactionData {
    fn from(data: Bytes) -> Self {
        Self(data)
    }
}
