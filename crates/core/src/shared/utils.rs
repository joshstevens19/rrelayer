use crate::network::ChainId;
use crate::shared::{bad_request, HttpError};
use crate::transaction::types::TransactionBlob;
use crate::{create_retry_client, rrelayer_error};
use alloy::primitives::U256;
use alloy_eips::eip4844::Blob;
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
) -> Result<Option<Vec<TransactionBlob>>, HttpError> {
    match blob_strings {
        Some(strings) => {
            let mut blobs = Vec::new();
            for blob_str in strings {
                let blob = blob_str.parse::<Blob>().map_err(|e| {
                    rrelayer_error!(
                        "Failed to parse blob it must be 131,072 bytes'{}': {:?}",
                        blob_str,
                        e
                    );
                    bad_request("Failed to parse blob it must be 131,072 bytes".to_string())
                })?;

                blobs.push(TransactionBlob::new(&blob));
            }
            Ok(Some(blobs))
        }
        None => Ok(None),
    }
}

pub fn format_wei_to_eth(wei: &U256) -> String {
    format_token_amount(wei, 18)
}

/// Formats a token amount to a human-readable string.
pub fn format_token_amount(amount: &U256, decimals: u8) -> String {
    let unit_divisor = U256::from(10u64.pow(decimals.into()));
    let whole_part = amount / unit_divisor;
    let remainder = amount % unit_divisor;

    if remainder.is_zero() {
        format!("{}", whole_part)
    } else {
        let decimal_str = format!("{:0width$}", remainder, width = decimals as usize);
        let decimal_trimmed = decimal_str.trim_end_matches('0');
        format!("{}.{}", whole_part, decimal_trimmed)
    }
}

pub async fn get_chain_id(provider_url: &str) -> Result<ChainId, String> {
    let provider = create_retry_client(provider_url)
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;
    let chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;

    Ok(ChainId::new(chain_id))
}
