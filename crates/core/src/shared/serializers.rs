use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serializer};

/// Serializes a SystemTime to RFC3339 formatted string.
///
/// Converts a SystemTime instance to an RFC3339 formatted string for JSON serialization.
///
/// # Arguments
/// * `time` - The SystemTime to serialize
/// * `serializer` - The serde serializer
///
/// # Returns
/// * `Ok(S::Ok)` - Successfully serialized time string
/// * `Err(S::Error)` - Serialization error
pub fn serialize_system_time<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(*time);
    let formatted = datetime.to_rfc3339();
    serializer.serialize_str(&formatted)
}

/// Serializes an optional SystemTime to RFC3339 formatted string or null.
///
/// Converts an optional SystemTime instance to either an RFC3339 formatted string
/// or null for JSON serialization.
///
/// # Arguments
/// * `time` - The optional SystemTime to serialize
/// * `serializer` - The serde serializer
///
/// # Returns
/// * `Ok(S::Ok)` - Successfully serialized time string or null
/// * `Err(S::Error)` - Serialization error
pub fn serialize_system_time_option<S>(
    time: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(time) = time {
        let datetime: DateTime<Utc> = DateTime::<Utc>::from(*time);
        let formatted = datetime.to_rfc3339();
        serializer.serialize_str(&formatted)
    } else {
        serializer.serialize_none()
    }
}

/// Deserializes an RFC3339 formatted string to SystemTime.
///
/// Converts an RFC3339 formatted string from JSON deserialization to a SystemTime instance.
///
/// # Arguments
/// * `deserializer` - The serde deserializer
///
/// # Returns
/// * `Ok(SystemTime)` - Successfully deserialized SystemTime
/// * `Err(D::Error)` - Deserialization error if string is not valid RFC3339
pub fn deserialize_system_time<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let datetime = DateTime::parse_from_rfc3339(&s).map_err(serde::de::Error::custom)?;
    Ok(datetime.with_timezone(&Utc).into())
}

/// Deserializes an optional RFC3339 formatted string to optional SystemTime.
///
/// Converts an optional RFC3339 formatted string from JSON deserialization to an optional SystemTime instance.
///
/// # Arguments
/// * `deserializer` - The serde deserializer
///
/// # Returns
/// * `Ok(Option<SystemTime>)` - Successfully deserialized optional SystemTime
/// * `Err(D::Error)` - Deserialization error if string is present but not valid RFC3339
pub fn deserialize_system_time_option<'de, D>(
    deserializer: D,
) -> Result<Option<SystemTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let datetime = DateTime::parse_from_rfc3339(&s).map_err(serde::de::Error::custom)?;
            Ok(Some(datetime.with_timezone(&Utc).into()))
        }
        None => Ok(None),
    }
}
