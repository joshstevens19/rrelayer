use alloy::primitives::U256;
use alloy_eips::eip7702::{Authorization, SignedAuthorization};
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

use crate::common_types::EvmAddress;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAuthorization {
    pub address: EvmAddress,
    pub chain_id: U256,
    pub nonce: u64,
    pub r: U256,
    pub s: U256,
    pub y_parity: u8,
}

impl From<&SignedAuthorization> for TransactionAuthorization {
    fn from(authorization: &SignedAuthorization) -> Self {
        Self {
            address: EvmAddress::new(authorization.address),
            chain_id: authorization.chain_id,
            nonce: authorization.nonce,
            r: authorization.r(),
            s: authorization.s(),
            y_parity: authorization.y_parity(),
        }
    }
}

impl From<SignedAuthorization> for TransactionAuthorization {
    fn from(authorization: SignedAuthorization) -> Self {
        (&authorization).into()
    }
}

impl From<&TransactionAuthorization> for SignedAuthorization {
    fn from(transaction_authorization: &TransactionAuthorization) -> Self {
        SignedAuthorization::new_unchecked(
            Authorization {
                address: transaction_authorization.address.into_address(),
                chain_id: transaction_authorization.chain_id,
                nonce: transaction_authorization.nonce,
            },
            transaction_authorization.y_parity,
            transaction_authorization.r,
            transaction_authorization.s,
        )
    }
}

impl From<TransactionAuthorization> for SignedAuthorization {
    fn from(transaction_authorization: TransactionAuthorization) -> Self {
        (&transaction_authorization).into()
    }
}

impl FromSql<'_> for TransactionAuthorization {
    fn from_sql(
        ty: &Type,
        raw: &[u8],
    ) -> Result<TransactionAuthorization, Box<dyn std::error::Error + Sync + Send>> {
        if !matches!(*ty, Type::JSON | Type::JSONB) {
            return Err(format!("Expected JSON or JSONB type, got {:?}", ty).into());
        }

        serde_json::from_slice::<TransactionAuthorization>(raw).map_err(Into::into)
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::JSON | Type::JSONB)
    }
}

impl ToSql for TransactionAuthorization {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        if !matches!(*ty, Type::JSON | Type::JSONB) {
            return Err(format!("Expected JSON or JSONB type, got {:?}", ty).into());
        }

        let serialized = serde_json::to_string(&self)?;
        out.extend_from_slice(serialized.as_bytes());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::JSON | Type::JSONB)
    }

    fn to_sql_checked(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        self.to_sql(ty, out)
    }
}
