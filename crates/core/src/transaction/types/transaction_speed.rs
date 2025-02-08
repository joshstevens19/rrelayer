use core::fmt;
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};

use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, Type};

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
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let speed = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

        match speed {
            "SLOW" => Ok(TransactionSpeed::Slow),
            "MEDIUM" => Ok(TransactionSpeed::Medium),
            "FAST" => Ok(TransactionSpeed::Fast),
            "SUPER" => Ok(TransactionSpeed::Super),
            _ => Err(format!("Unknown TransactionSpeed: {}", speed).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}
