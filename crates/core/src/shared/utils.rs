use crate::rrelayer_error;
use alloy_eips::eip4844::Blob;
use axum::http::StatusCode;
use std::time::Duration;
use tokio::time::sleep;

pub fn option_if<T>(condition: bool, value: T) -> Option<T> {
    if condition {
        Some(value)
    } else {
        None
    }
}

pub async fn sleep_ms(ms: &u64) {
    sleep(Duration::from_millis(*ms)).await
}

pub fn convert_blob_strings_to_blobs(
    blob_strings: Option<Vec<String>>,
) -> Result<Option<Vec<Blob>>, StatusCode> {
    match blob_strings {
        Some(strings) => {
            let mut blobs = Vec::new();
            for blob_str in strings {
                let blob = blob_str.parse::<Blob>().map_err(|e| {
                    rrelayer_error!("Failed to parse blob hex string '{}': {:?}", blob_str, e);
                    StatusCode::BAD_REQUEST
                })?;

                blobs.push(blob);
            }
            Ok(Some(blobs))
        }
        None => Ok(None),
    }
}
