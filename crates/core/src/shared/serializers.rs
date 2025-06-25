use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize_system_time<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(*time);
    let formatted = datetime.to_rfc3339();
    serializer.serialize_str(&formatted)
}

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

pub fn deserialize_system_time<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let datetime = DateTime::parse_from_rfc3339(&s)
        .map_err(serde::de::Error::custom)?;
    Ok(datetime.with_timezone(&Utc).into())
}

pub fn deserialize_system_time_option<'de, D>(deserializer: D) -> Result<Option<SystemTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let datetime = DateTime::parse_from_rfc3339(&s)
                .map_err(serde::de::Error::custom)?;
            Ok(Some(datetime.with_timezone(&Utc).into()))
        }
        None => Ok(None),
    }
}