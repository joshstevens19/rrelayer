use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Serializer;

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
