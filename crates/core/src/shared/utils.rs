use crate::rrelayer_error;
use crate::transaction::types::TransactionBlob;
use alloy::primitives::U256;
use alloy_eips::eip4844::Blob;
use axum::http::StatusCode;
use std::time::Duration;
use tokio::time::sleep;

/// Returns Some(value) if condition is true, otherwise None.
pub fn option_if<T>(condition: bool, value: T) -> Option<T> {
    if condition {
        Some(value)
    } else {
        None
    }
}

/// Asynchronously sleeps for the specified number of milliseconds.
pub async fn sleep_ms(ms: &u64) {
    sleep(Duration::from_millis(*ms)).await
}

/// Converts optional blob strings to optional blob objects.
pub fn convert_blob_strings_to_blobs(
    blob_strings: Option<Vec<String>>,
) -> Result<Option<Vec<TransactionBlob>>, StatusCode> {
    match blob_strings {
        Some(strings) => {
            let mut blobs = Vec::new();
            for blob_str in strings {
                let blob = blob_str.parse::<Blob>().map_err(|e| {
                    rrelayer_error!("Failed to parse blob hex string '{}': {:?}", blob_str, e);
                    StatusCode::BAD_REQUEST
                })?;

                blobs.push(TransactionBlob::new(&blob));
            }
            Ok(Some(blobs))
        }
        None => Ok(None),
    }
}

/// Formats a Wei amount to a human-readable ETH string.
pub fn format_wei_to_eth(wei: &U256) -> String {
    let eth_divisor = U256::from(10u64.pow(18));
    let whole_eth = wei / eth_divisor;
    let remainder = wei % eth_divisor;

    if remainder.is_zero() {
        format!("{}", whole_eth)
    } else {
        let decimal_str = format!("{:018}", remainder);
        let decimal_trimmed = decimal_str.trim_end_matches('0');
        format!("{}.{}", whole_eth, decimal_trimmed)
    }
}

/// Formats a token amount to a human-readable string.
pub fn format_token_amount(amount: &U256) -> String {
    // For now, use the same formatting as ETH (18 decimals)
    // This can be enhanced to support different token decimals
    format_wei_to_eth(amount)
}
