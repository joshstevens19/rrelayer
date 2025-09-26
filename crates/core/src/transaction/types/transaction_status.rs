use core::fmt;
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum TransactionStatus {
    PENDING,
    INMEMPOOL,
    MINED,
    CONFIRMED,
    FAILED,
    EXPIRED,
}

impl Display for TransactionStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl TransactionStatus {
    fn format(&self) -> String {
        match self {
            TransactionStatus::PENDING => "PENDING".to_string(),
            TransactionStatus::INMEMPOOL => "INMEMPOOL".to_string(),
            TransactionStatus::MINED => "MINED".to_string(),
            TransactionStatus::CONFIRMED => "CONFIRMED".to_string(),
            TransactionStatus::FAILED => "FAILED".to_string(),
            TransactionStatus::EXPIRED => "EXPIRED".to_string(),
        }
    }
}

impl<'a> FromSql<'a> for TransactionStatus {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "tx_status" {
            let status =
                from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match status {
                "PENDING" => Ok(TransactionStatus::PENDING),
                "INMEMPOOL" => Ok(TransactionStatus::INMEMPOOL),
                "MINED" => Ok(TransactionStatus::MINED),
                "CONFIRMED" => Ok(TransactionStatus::CONFIRMED),
                "FAILED" => Ok(TransactionStatus::FAILED),
                "EXPIRED" => Ok(TransactionStatus::EXPIRED),
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
                "PENDING" => Ok(TransactionStatus::PENDING),
                "INMEMPOOL" => Ok(TransactionStatus::INMEMPOOL),
                "MINED" => Ok(TransactionStatus::MINED),
                "CONFIRMED" => Ok(TransactionStatus::CONFIRMED),
                "FAILED" => Ok(TransactionStatus::FAILED),
                "EXPIRED" => Ok(TransactionStatus::EXPIRED),
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
            TransactionStatus::PENDING => "PENDING",
            TransactionStatus::INMEMPOOL => "INMEMPOOL",
            TransactionStatus::MINED => "MINED",
            TransactionStatus::CONFIRMED => "CONFIRMED",
            TransactionStatus::FAILED => "FAILED",
            TransactionStatus::EXPIRED => "EXPIRED",
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
