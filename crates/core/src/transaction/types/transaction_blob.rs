use alloy::primitives::FixedBytes;
use alloy_eips::eip4844::{Blob, BYTES_PER_BLOB};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, str::FromStr};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionBlob(Vec<u8>);

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
        &self.0.as_slice()
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

impl ToString for TransactionBlob {
    fn to_string(&self) -> String {
        self.to_hex()
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::FixedBytes;

    #[test]
    fn test_transaction_blob_new() {
        let blob_data = [1u8; BYTES_PER_BLOB];
        let blob = FixedBytes::from(blob_data);
        let transaction_blob = TransactionBlob::new(&blob);

        assert_eq!(transaction_blob.0, blob_data.to_vec());
    }

    #[test]
    fn test_transaction_blob_to_blob() {
        let blob_data = [42u8; BYTES_PER_BLOB];
        let transaction_blob = TransactionBlob(blob_data.to_vec());
        let blob = transaction_blob.to_blob().unwrap();

        assert_eq!(blob.as_slice(), &blob_data);
    }

    #[test]
    fn test_transaction_blob_invalid_length() {
        let invalid_data = vec![1u8; 100]; // Wrong length
        let transaction_blob = TransactionBlob(invalid_data);

        assert!(transaction_blob.to_blob().is_err());
    }

    #[test]
    fn test_transaction_blob_from_hex() {
        let hex_data = "0x".to_string() + &"aa".repeat(BYTES_PER_BLOB);
        let transaction_blob = TransactionBlob::from_hex(&hex_data).unwrap();

        assert_eq!(transaction_blob.0, vec![0xaa; BYTES_PER_BLOB]);
    }

    #[test]
    fn test_transaction_blob_to_hex() {
        let data = vec![0xbb; BYTES_PER_BLOB];
        let transaction_blob = TransactionBlob(data);
        let hex = transaction_blob.to_hex();

        assert_eq!(hex, "0x".to_string() + &"bb".repeat(BYTES_PER_BLOB));
    }

    #[test]
    fn test_transaction_blob_from_str() {
        let hex_data = "0x".to_string() + &"cc".repeat(BYTES_PER_BLOB);
        let transaction_blob: TransactionBlob = hex_data.parse().unwrap();

        assert_eq!(transaction_blob.0, vec![0xcc; BYTES_PER_BLOB]);
    }

    #[test]
    fn test_transaction_blob_conversions() {
        let blob_data = [123u8; BYTES_PER_BLOB];
        let original_blob = FixedBytes::from(blob_data);

        // Test From<Blob> for TransactionBlob
        let transaction_blob: TransactionBlob = original_blob.into();

        // Test TryFrom<TransactionBlob> for Blob
        let recovered_blob: Blob = transaction_blob.try_into().unwrap();

        assert_eq!(original_blob, recovered_blob);
    }
}
