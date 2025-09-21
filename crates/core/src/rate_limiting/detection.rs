use crate::common_types::EvmAddress;
use axum::http::HeaderMap;

use super::types::{
    RateLimitDetectContext, RateLimitDetectMethod, RateLimitError, TransactionType,
};

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
            return Ok(RateLimitDetectContext {
                key: header_value.to_string(),
                detection_method: RateLimitDetectMethod::Header,
                transaction_type: TransactionType::Direct,
            });
        }

        if self.fallback_to_relayer {
            return Ok(RateLimitDetectContext {
                key: format!("{:?}", relayer_address),
                detection_method: RateLimitDetectMethod::Fallback,
                transaction_type: TransactionType::Direct,
            });
        }

        Err(RateLimitError::Detection("No rate limit key found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use std::str::FromStr;

    #[test]
    fn test_header_detection() {
        let detector = RateLimitDetector::new(false);

        let mut headers = HeaderMap::new();
        headers.insert("x-rrelayer-rate-limit-key", "user-123".parse().unwrap());

        let relayer_address =
            EvmAddress::from_str("0x742d35Cc6aF6C5C8c3C4B4C8e1A36F1c57F1b8Ff").unwrap();
        let result = detector.detect(&headers, &relayer_address).unwrap();

        assert_eq!(result.key, "user-123");
        assert!(matches!(result.detection_method, RateLimitDetectMethod::Header));
    }

    #[test]
    fn test_fallback_detection() {
        let detector = RateLimitDetector::new(true);

        let headers = HeaderMap::new();
        let relayer_address =
            EvmAddress::from_str("0x1234567890123456789012345678901234567890").unwrap();

        let result = detector.detect(&headers, &relayer_address).unwrap();

        assert_eq!(result.key, format!("{:?}", relayer_address));
        assert!(matches!(result.detection_method, RateLimitDetectMethod::Fallback));
    }
}
