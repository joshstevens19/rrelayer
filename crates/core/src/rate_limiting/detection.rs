use crate::common_types::EvmAddress;
use axum::http::HeaderMap;

use super::types::{RateLimitDetectContext, RateLimitError};

pub const RATE_LIMIT_HEADER_NAME: &str = "x-rrelayer-rate-limit-key";

pub struct RateLimitDetector {
    fallback_to_relayer: bool,
}

impl RateLimitDetector {
    pub fn new(fallback_to_relayer: bool) -> Self {
        Self { fallback_to_relayer }
    }

    pub fn detect(
        &self,
        headers: &HeaderMap,
        relayer_address: &EvmAddress,
    ) -> Result<RateLimitDetectContext, RateLimitError> {
        if let Some(header_value) =
            headers.get(RATE_LIMIT_HEADER_NAME).and_then(|v| v.to_str().ok())
        {
            return Ok(RateLimitDetectContext { key: header_value.to_string() });
        }

        if self.fallback_to_relayer {
            return Ok(RateLimitDetectContext { key: relayer_address.hex() });
        }

        Err(RateLimitError::NoRateLimitKey)
    }
}
