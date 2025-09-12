use std::error::Error;
use std::fmt::Display;
use std::str::FromStr;

use crate::common_types::EvmAddress;
use crate::postgres::ToSql;
use alloy::primitives::{FixedBytes, U256};
use alloy::{
    hex,
    primitives::{Bytes, PrimitiveSignature, B256},
};
use bytes::BytesMut;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, Type};

#[derive(Clone, Debug, PartialEq, Copy)]
pub struct Signature(PrimitiveSignature);

impl Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Signature {
    pub fn to_hex(&self) -> String {
        let bytes = self.0.as_bytes();
        hex::encode(bytes)
    }

    pub fn to_bytes(&self) -> Bytes {
        self.0.as_bytes().into()
    }

    pub fn recover_address_from_msg(
        &self,
        msg: &str,
    ) -> Result<EvmAddress, alloy::primitives::SignatureError> {
        Ok(EvmAddress::from(self.0.recover_address_from_msg(msg)?))
    }

    pub fn recover_address_from_prehash(
        &self,
        prehash: &B256,
    ) -> Result<EvmAddress, alloy::primitives::SignatureError> {
        Ok(EvmAddress::from(self.0.recover_address_from_prehash(prehash)?))
    }

    pub fn inner(&self) -> &PrimitiveSignature {
        &self.0
    }

    pub fn to_eip712_tuple(&self, deadline: U256) -> (u8, FixedBytes<32>, FixedBytes<32>, U256) {
        (if self.0.v() { 28u8 } else { 27u8 }, self.0.r().into(), self.0.s().into(), deadline)
    }

    pub fn v(&self) -> u8 {
        if self.0.v() {
            28u8
        } else {
            27u8
        }
    }

    pub fn r(&self) -> FixedBytes<32> {
        self.0.r().into()
    }

    pub fn s(&self) -> FixedBytes<32> {
        self.0.s().into()
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_hex().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let sig = PrimitiveSignature::from_str(&s)
            .map_err(|e| serde::de::Error::custom(format!("Invalid signature: {e}")))?;
        Ok(Signature(sig))
    }
}

impl From<PrimitiveSignature> for Signature {
    fn from(sig: PrimitiveSignature) -> Self {
        Signature(sig)
    }
}

impl FromStr for Signature {
    type Err = alloy::primitives::SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PrimitiveSignature::from_str(s).map(Signature)
    }
}

impl<'a> FromSql<'a> for Signature {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        // Ethereum signatures are 65 bytes (32 bytes r + 32 bytes s + 1 byte v)
        if raw.len() != 65 {
            return Err("Invalid byte length for Ethereum signature".into());
        }

        let sig = PrimitiveSignature::try_from(raw)
            .map_err(|e| format!("Failed to parse signature: {e}"))?;

        Ok(Signature(sig))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }
}

impl ToSql for Signature {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let sig_bytes = self.0.as_bytes();
        out.extend_from_slice(&sig_bytes);
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    to_sql_checked!();
}
