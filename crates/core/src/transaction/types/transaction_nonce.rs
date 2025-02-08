use std::{
    error::Error,
    fmt::Display,
    hash::{Hash, Hasher},
    ops::Add,
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct TransactionNonce(u64);

impl TransactionNonce {
    pub fn new(nonce: u64) -> Self {
        TransactionNonce(nonce)
    }
}

impl Hash for TransactionNonce {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TransactionNonce {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Add<i32> for TransactionNonce {
    type Output = Self;

    fn add(self, other: i32) -> Self {
        TransactionNonce(self.0 + other as u64)
    }
}

impl<'a> FromSql<'a> for TransactionNonce {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        i64::from_sql(ty, raw).map(|value| TransactionNonce::from(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT8
    }
}

impl ToSql for TransactionNonce {
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
pub struct ParseTransactionNonceError;

impl Display for ParseTransactionNonceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid chain id")
    }
}

impl Error for ParseTransactionNonceError {}

impl FromStr for TransactionNonce {
    type Err = ParseTransactionNonceError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        u64::from_str(param).map(TransactionNonce).map_err(|_| ParseTransactionNonceError)
    }
}

impl From<TransactionNonce> for u64 {
    fn from(nonce: TransactionNonce) -> Self {
        nonce.0
    }
}

impl From<u64> for TransactionNonce {
    fn from(nonce: u64) -> Self {
        TransactionNonce(nonce)
    }
}

impl From<i64> for TransactionNonce {
    fn from(nonce: i64) -> Self {
        TransactionNonce(nonce as u64)
    }
}

impl From<TransactionNonce> for i64 {
    fn from(nonce: TransactionNonce) -> Self {
        nonce.0 as i64
    }
}
