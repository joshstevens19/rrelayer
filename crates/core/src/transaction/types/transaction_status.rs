use core::fmt;
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum TransactionStatus {
    Pending,
    Inmempool,
    Mined,
    Confirmed,
    Failed,
    Expired,
}

impl Display for TransactionStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl TransactionStatus {
    fn format(&self) -> String {
        match self {
            TransactionStatus::Pending => "PENDING".to_string(),
            TransactionStatus::Inmempool => "INMEMPOOL".to_string(),
            TransactionStatus::Mined => "MINED".to_string(),
            TransactionStatus::Confirmed => "CONFIRMED".to_string(),
            TransactionStatus::Failed => "FAILED".to_string(),
            TransactionStatus::Expired => "EXPIRED".to_string(),
        }
    }
}

impl<'a> FromSql<'a> for TransactionStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "tx_status" {
            let status =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match status {
                "PENDING" => Ok(TransactionStatus::Pending),
                "INMEMPOOL" => Ok(TransactionStatus::Inmempool),
                "MINED" => Ok(TransactionStatus::Mined),
                "CONFIRMED" => Ok(TransactionStatus::Confirmed),
                "FAILED" => Ok(TransactionStatus::Failed),
                "EXPIRED" => Ok(TransactionStatus::Expired),
                _ => Err(format!("Unknown TransactionStatus: {}", status).into()),
            }
        } else if *ty == Type::TEXT
            || *ty == Type::CHAR
            || *ty == Type::VARCHAR
            || *ty == Type::BPCHAR
        {
            let status =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match status {
                "PENDING" => Ok(TransactionStatus::Pending),
                "INMEMPOOL" => Ok(TransactionStatus::Inmempool),
                "MINED" => Ok(TransactionStatus::Mined),
                "CONFIRMED" => Ok(TransactionStatus::Confirmed),
                "FAILED" => Ok(TransactionStatus::Failed),
                "EXPIRED" => Ok(TransactionStatus::Expired),
                _ => Err(format!("Unknown TransactionStatus: {}", status).into()),
            }
        } else {
            Err(format!("Unexpected type for TransactionStatus: {}", ty).into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "tx_status")
    }
}

impl ToSql for TransactionStatus {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for TransactionStatus: {}", ty).into());
        }

        let status_str = match self {
            TransactionStatus::Pending => "PENDING",
            TransactionStatus::Inmempool => "INMEMPOOL",
            TransactionStatus::Mined => "MINED",
            TransactionStatus::Confirmed => "CONFIRMED",
            TransactionStatus::Failed => "FAILED",
            TransactionStatus::Expired => "EXPIRED",
        };

        out.extend_from_slice(status_str.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "tx_status")
    }

    tokio_postgres::types::to_sql_checked!();
}
