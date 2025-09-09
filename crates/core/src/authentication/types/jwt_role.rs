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
    /// Formats the JWT role as a string representation.
    ///
    /// Converts the role enum variant to its corresponding uppercase string format
    /// used in the database and external representations.
    ///
    /// # Returns
    /// * `String` - The uppercase string representation of the role:
    ///   - Admin -> "ADMIN"
    ///   - ReadOnly -> "READONLY"
    ///   - Manager -> "MANAGER"
    ///   - Integrator -> "INTEGRATOR"
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
    /// Deserializes a JWT role from PostgreSQL database types.
    ///
    /// Converts raw bytes from PostgreSQL into a JwtRole enum variant.
    /// Supports both custom "user_role" enum type and standard text types.
    ///
    /// # Arguments
    /// * `ty` - The PostgreSQL type information
    /// * `raw` - The raw bytes from the database
    ///
    /// # Returns
    /// * `Ok(Self)` - The parsed JwtRole if the value is recognized
    /// * `Err(Box<dyn Error + Sync + Send>)` - If parsing fails due to invalid UTF-8,
    ///   unknown role string, or unsupported PostgreSQL type
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if ty.name() == "user_role" {
            let role = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match role {
                "ADMIN" => Ok(JwtRole::Admin),
                "READONLY" => Ok(JwtRole::ReadOnly),
                "MANAGER" => Ok(JwtRole::Manager),
                "INTEGRATOR" => Ok(JwtRole::Integrator),
                _ => Err(format!("Unknown JwtRole: {}", role).into()),
            }
        } else if *ty == Type::TEXT
            || *ty == Type::CHAR
            || *ty == Type::VARCHAR
            || *ty == Type::BPCHAR
        {
            let role = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

            match role {
                "ADMIN" => Ok(JwtRole::Admin),
                "READONLY" => Ok(JwtRole::ReadOnly),
                "MANAGER" => Ok(JwtRole::Manager),
                "INTEGRATOR" => Ok(JwtRole::Integrator),
                _ => Err(format!("Unknown JwtRole: {}", role).into()),
            }
        } else {
            Err(format!("Unexpected type for JwtRole: {}", ty).into())
        }
    }

    /// Checks if the PostgreSQL type is supported for deserialization.
    ///
    /// Determines whether the given PostgreSQL type can be converted to a JwtRole.
    /// Supports text types (TEXT, CHAR, VARCHAR, BPCHAR) and the custom "user_role" enum.
    ///
    /// # Arguments
    /// * `ty` - The PostgreSQL type to check
    ///
    /// # Returns
    /// * `true` - If the type can be converted to JwtRole
    /// * `false` - If the type is not supported
    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "user_role")
    }
}

impl ToSql for JwtRole {
    /// Serializes a JWT role to PostgreSQL database format.
    ///
    /// Converts a JwtRole enum variant to its string representation and writes
    /// it to the output buffer for storage in PostgreSQL.
    ///
    /// # Arguments
    /// * `ty` - The target PostgreSQL type
    /// * `out` - The output buffer to write the serialized data to
    ///
    /// # Returns
    /// * `Ok(IsNull::No)` - If serialization succeeds (never null)
    /// * `Err(Box<dyn Error + Sync + Send>)` - If the target type is not supported
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for JwtRole: {}", ty).into());
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

    /// Checks if the PostgreSQL type is supported for serialization.
    ///
    /// Determines whether the given PostgreSQL type can accept a JwtRole value.
    /// Supports text types (TEXT, CHAR, VARCHAR, BPCHAR) and the custom "user_role" enum.
    ///
    /// # Arguments
    /// * `ty` - The PostgreSQL type to check
    ///
    /// # Returns
    /// * `true` - If the type can accept JwtRole values
    /// * `false` - If the type is not supported
    fn accepts(ty: &Type) -> bool {
        (*ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR)
            || (ty.name() == "user_role")
    }

    tokio_postgres::types::to_sql_checked!();
}
