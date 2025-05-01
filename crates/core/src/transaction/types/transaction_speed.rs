use core::fmt;
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, Type};

use crate::postgres::ToSql;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum TransactionSpeed {
    Slow,
    Medium,
    Fast,
    Super,
}

impl Display for TransactionSpeed {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl TransactionSpeed {
    pub fn format(&self) -> String {
        match self {
            TransactionSpeed::Slow => "SLOW".to_string(),
            TransactionSpeed::Medium => "MEDIUM".to_string(),
            TransactionSpeed::Fast => "FAST".to_string(),
            TransactionSpeed::Super => "SUPER".to_string(),
        }
    }
}

impl<'a> FromSql<'a> for TransactionSpeed {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "speed" {
            let speed = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match speed {
                "SLOW" => Ok(TransactionSpeed::Slow),
                "MEDIUM" => Ok(TransactionSpeed::Medium),
                "FAST" => Ok(TransactionSpeed::Fast),
                "SUPER" => Ok(TransactionSpeed::Super),
                _ => Err(format!("Unknown TransactionSpeed: {}", speed).into()),
            }
        } else if *ty == Type::TEXT ||
            *ty == Type::CHAR ||
            *ty == Type::VARCHAR ||
            *ty == Type::BPCHAR
        {
            // Handle text types for backward compatibility
            let speed = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match speed {
                "SLOW" => Ok(TransactionSpeed::Slow),
                "MEDIUM" => Ok(TransactionSpeed::Medium),
                "FAST" => Ok(TransactionSpeed::Fast),
                "SUPER" => Ok(TransactionSpeed::Super),
                _ => Err(format!("Unknown TransactionSpeed: {}", speed).into()),
            }
        } else {
            Err(format!("Unexpected type for TransactionSpeed: {}", ty).into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR) ||
            (ty.name() == "speed")
    }
}

impl ToSql for TransactionSpeed {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for TransactionSpeed: {}", ty).into());
        }

        let speed_str = match self {
            TransactionSpeed::Slow => "SLOW",
            TransactionSpeed::Medium => "MEDIUM",
            TransactionSpeed::Fast => "FAST",
            TransactionSpeed::Super => "SUPER",
        };

        out.extend_from_slice(speed_str.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR) ||
            (ty.name() == "speed")
    }

    tokio_postgres::types::to_sql_checked!();
}
