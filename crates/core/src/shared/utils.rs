use crate::network::ChainId;
use crate::shared::{bad_request, HttpError};
use crate::transaction::types::TransactionBlob;
use crate::{create_retry_client, rrelayer_error};
use alloy::primitives::U256;
use alloy::providers::Provider;
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
                    rrelayer_error!("Failed to parse blob hex string '{}': {:?}", blob_str, e);
                    bad_request("Failed to parse blob hex string".to_string())
                })?;

                blobs.push(TransactionBlob::new(&blob));
            }
            Ok(Some(blobs))
        }
        None => Ok(None),
    }
}

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

pub async fn get_chain_id(provider_url: &str) -> Result<ChainId, String> {
    let provider = create_retry_client(&provider_url)
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;
    let chain_id = provider
        .get_chain_id()
        .await
        .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;

    Ok(ChainId::new(chain_id))
}
