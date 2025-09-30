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
    SLOW,
    MEDIUM,
    FAST,
    SUPER,
}

impl Display for TransactionSpeed {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl TransactionSpeed {
    pub fn format(&self) -> String {
        match self {
            TransactionSpeed::SLOW => "SLOW".to_string(),
            TransactionSpeed::MEDIUM => "MEDIUM".to_string(),
            TransactionSpeed::FAST => "FAST".to_string(),
            TransactionSpeed::SUPER => "SUPER".to_string(),
        }
    }

    pub fn next_speed(&self) -> Option<TransactionSpeed> {
        match self {
            TransactionSpeed::SLOW => Some(TransactionSpeed::MEDIUM),
            TransactionSpeed::MEDIUM => Some(TransactionSpeed::FAST),
            TransactionSpeed::FAST => Some(TransactionSpeed::SUPER),
            TransactionSpeed::SUPER => None,
        }
    }
}

impl<'a> FromSql<'a> for TransactionSpeed {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "speed" {
            let speed = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match speed {
                "SLOW" => Ok(TransactionSpeed::SLOW),
                "MEDIUM" => Ok(TransactionSpeed::MEDIUM),
                "FAST" => Ok(TransactionSpeed::FAST),
                "SUPER" => Ok(TransactionSpeed::SUPER),
                _ => Err(format!("Unknown TransactionSpeed: {}", speed).into()),
            }
        } else if *ty == Type::TEXT
            || *ty == Type::CHAR
            || *ty == Type::VARCHAR
            || *ty == Type::BPCHAR
        {
            let speed = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match speed {
                "SLOW" => Ok(TransactionSpeed::SLOW),
                "MEDIUM" => Ok(TransactionSpeed::MEDIUM),
                "FAST" => Ok(TransactionSpeed::FAST),
                "SUPER" => Ok(TransactionSpeed::SUPER),
                _ => Err(format!("Unknown TransactionSpeed: {}", speed).into()),
            }
        } else {
            Err(format!("Unexpected type for TransactionSpeed: {}", ty).into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "speed")
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
            TransactionSpeed::SLOW => "SLOW",
            TransactionSpeed::MEDIUM => "MEDIUM",
            TransactionSpeed::FAST => "FAST",
            TransactionSpeed::SUPER => "SUPER",
        };

        out.extend_from_slice(speed_str.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "speed")
    }

    tokio_postgres::types::to_sql_checked!();
}
