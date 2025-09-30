use alloy::primitives::FixedBytes;
use alloy_eips::eip4844::{Blob, BYTES_PER_BLOB};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::{convert::TryFrom, str::FromStr};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionBlob(Vec<u8>);

impl Display for TransactionBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl TransactionBlob {
    pub fn new(blob: &Blob) -> Self {
        TransactionBlob(blob.as_slice().to_vec())
    }

    pub fn to_blob(&self) -> Result<Blob> {
        if self.0.len() != BYTES_PER_BLOB {
            return Err(anyhow!(
                "Invalid blob length: expected {}, got {}",
                BYTES_PER_BLOB,
                self.0.len()
            ));
        }

        let mut bytes = [0u8; BYTES_PER_BLOB];
        bytes.copy_from_slice(&self.0);
        Ok(FixedBytes::from(bytes))
    }

    pub fn from_hex(hex: &str) -> Result<Self> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);

        if hex.len() != BYTES_PER_BLOB * 2 {
            return Err(anyhow!(
                "Invalid hex length for blob: expected {}, got {}",
                BYTES_PER_BLOB * 2,
                hex.len()
            ));
        }

        let bytes = hex::decode(hex).map_err(|e| anyhow!("Failed to decode hex string: {}", e))?;

        Ok(TransactionBlob(bytes))
    }

    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.0))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<Blob> for TransactionBlob {
    fn from(blob: Blob) -> Self {
        TransactionBlob::new(&blob)
    }
}

impl TryFrom<TransactionBlob> for Blob {
    type Error = anyhow::Error;

    fn try_from(transaction_blob: TransactionBlob) -> Result<Self> {
        transaction_blob.to_blob()
    }
}

impl TransactionBlob {
    pub fn from_blobs(blobs: &[Blob]) -> Vec<TransactionBlob> {
        blobs.iter().map(|blob| TransactionBlob::from(*blob)).collect()
    }

    pub fn to_blobs(transaction_blobs: &[TransactionBlob]) -> Result<Vec<Blob>> {
        transaction_blobs.iter().map(|tb| tb.to_blob()).collect()
    }
}

impl FromStr for TransactionBlob {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        TransactionBlob::from_hex(s)
    }
}

impl FromSql<'_> for TransactionBlob {
    fn from_sql(
        ty: &Type,
        raw: &[u8],
    ) -> Result<TransactionBlob, Box<dyn std::error::Error + Sync + Send>> {
        if *ty != Type::BYTEA {
            return Err(format!("Expected BYTEA type, got {:?}", ty).into());
        }

        if raw.len() != BYTES_PER_BLOB {
            return Err(
                format!("Expected {} bytes for blob, got {}", BYTES_PER_BLOB, raw.len()).into()
            );
        }

        Ok(TransactionBlob(raw.to_vec()))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }
}

impl ToSql for TransactionBlob {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        if *ty != Type::BYTEA {
            return Err(format!("Expected BYTEA type, got {:?}", ty).into());
        }

        out.extend_from_slice(&self.0);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    fn to_sql_checked(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.to_sql(ty, out)
    }
}
