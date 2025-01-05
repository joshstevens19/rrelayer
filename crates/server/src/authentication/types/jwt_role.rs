use core::fmt;
use std::{
    error::Error,
    fmt::{Display, Formatter},
    str::from_utf8,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum JwtRole {
    Admin,
    Manager,
    Integrator,
    ReadOnly,
}

impl Display for JwtRole {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl JwtRole {
    pub fn format(&self) -> String {
        match self {
            JwtRole::Admin => "ADMIN".to_string(),
            JwtRole::ReadOnly => "READONLY".to_string(),
            JwtRole::Manager => "MANAGER".to_string(),
            JwtRole::Integrator => "INTEGRATOR".to_string(),
        }
    }
}

impl<'a> FromSql<'a> for JwtRole {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let role = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

        match role {
            "ADMIN" => Ok(JwtRole::Admin),
            "READONLY" => Ok(JwtRole::ReadOnly),
            "MANAGER" => Ok(JwtRole::Manager),
            "INTEGRATOR" => Ok(JwtRole::Integrator),
            _ => Err(format!("Unknown JwtRole: {}", role).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for JwtRole {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type: {}", ty).into());
        }

        let status_str = match self {
            JwtRole::Admin => "ADMIN",
            JwtRole::ReadOnly => "READONLY",
            JwtRole::Manager => "MANAGER",
            JwtRole::Integrator => "INTEGRATOR",
        };

        out.extend_from_slice(status_str.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }

    tokio_postgres::types::to_sql_checked!();
}
