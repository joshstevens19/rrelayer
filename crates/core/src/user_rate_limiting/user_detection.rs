use crate::common_types::EvmAddress;
use crate::yaml::UserDetectionConfig;
use alloy::primitives::Address;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_address: EvmAddress,
    pub detection_method: UserDetectionMethod,
    pub transaction_type: TransactionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserDetectionMethod {
    Header,
    Eip2771,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Direct,
    Gasless,
    Automated,
}

#[derive(Debug, Error)]
pub enum UserDetectionError {
    #[error("Invalid address format: {0}")]
    InvalidAddress(String),

    #[error("Header parsing error: {0}")]
    HeaderError(String),

    #[error("EIP-2771 parsing error: {0}")]
    Eip2771Error(String),

    #[error("No user detection method succeeded")]
    NoDetectionMethod,
}

pub struct UserDetector {
    config: UserDetectionConfig,
}

impl UserDetector {
    pub fn new(config: UserDetectionConfig) -> Self {
        Self { config }
    }

    /// Attempts to detect the end user from various sources
    pub fn detect_user(
        &self,
        headers: &HeaderMap,
        transaction_to: Option<&EvmAddress>,
        transaction_data: &[u8],
        fallback_address: &EvmAddress,
    ) -> Result<UserContext, UserDetectionError> {
        // Try header detection first
        if self.config.enable_header_detection {
            if let Ok(user_context) = self.detect_from_header(headers) {
                return Ok(user_context);
            }
        }

        // Try EIP-2771 detection
        if self.config.enable_eip2771_parsing {
            if let Ok(user_context) = self.detect_from_eip2771(transaction_to, transaction_data) {
                return Ok(user_context);
            }
        }

        // Fallback to relayer if configured
        if self.config.fallback_to_relayer {
            return Ok(UserContext {
                user_address: *fallback_address,
                detection_method: UserDetectionMethod::Fallback,
                transaction_type: TransactionType::Direct,
            });
        }

        Err(UserDetectionError::NoDetectionMethod)
    }

    /// Detect user from HTTP headers
    fn detect_from_header(&self, headers: &HeaderMap) -> Result<UserContext, UserDetectionError> {
        let header_value = headers
            .get(&self.config.header_name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| UserDetectionError::HeaderError("Header not found".to_string()))?;

        let user_address = EvmAddress::from_str(header_value)
            .map_err(|e| UserDetectionError::InvalidAddress(e.to_string()))?;

        // Check for transaction type header
        let transaction_type = headers
            .get("X-Transaction-Type")
            .and_then(|v| v.to_str().ok())
            .map(|s| match s.to_lowercase().as_str() {
                "gasless" => TransactionType::Gasless,
                "automated" => TransactionType::Automated,
                _ => TransactionType::Direct,
            })
            .unwrap_or(TransactionType::Direct);

        Ok(UserContext {
            user_address,
            detection_method: UserDetectionMethod::Header,
            transaction_type,
        })
    }

    /// Detect user from EIP-2771 meta-transaction format
    fn detect_from_eip2771(
        &self,
        transaction_to: Option<&EvmAddress>,
        transaction_data: &[u8],
    ) -> Result<UserContext, UserDetectionError> {
        // Check if transaction is to a trusted forwarder
        if let Some(to_address) = transaction_to {
            if let Some(ref trusted_forwarders) = self.config.trusted_forwarders {
                let to_str = format!("{:?}", to_address);
                if !trusted_forwarders
                    .iter()
                    .any(|forwarder| forwarder.eq_ignore_ascii_case(&to_str))
                {
                    return Err(UserDetectionError::Eip2771Error(
                        "Transaction not to trusted forwarder".to_string(),
                    ));
                }
            }
        }

        // EIP-2771: Extract original sender from calldata
        // Format: execute(address from, bytes data) where 'from' is the original sender
        if transaction_data.len() < 4 {
            return Err(UserDetectionError::Eip2771Error("Insufficient calldata".to_string()));
        }

        // Check for common EIP-2771 function selectors
        let function_selector = &transaction_data[0..4];
        let user_address = match function_selector {
            // execute(address,bytes) - 0x0e43319d
            [0x0e, 0x43, 0x31, 0x9d] => self.parse_execute_function(transaction_data)?,
            // executeWithProof(address,bytes,bytes32[]) - example selector
            [0x1f, 0x4d, 0x5e, 0x7a] => self.parse_execute_with_proof_function(transaction_data)?,
            // executeTypedData(...) - another common pattern
            _ => {
                // Try generic parsing - look for address in first parameter
                self.parse_generic_eip2771(transaction_data)?
            }
        };

        Ok(UserContext {
            user_address,
            detection_method: UserDetectionMethod::Eip2771,
            transaction_type: TransactionType::Gasless,
        })
    }

    /// Parse execute(address from, bytes data) function
    fn parse_execute_function(&self, data: &[u8]) -> Result<EvmAddress, UserDetectionError> {
        if data.len() < 36 {
            return Err(UserDetectionError::Eip2771Error(
                "Invalid execute calldata length".to_string(),
            ));
        }

        // Skip function selector (4 bytes) and extract first parameter (address)
        let address_bytes = &data[4..36];

        // Address is in the last 20 bytes of the 32-byte parameter
        let address_start = 12; // 32 - 20 = 12
        let address_bytes = &address_bytes[address_start..];

        if address_bytes.len() != 20 {
            return Err(UserDetectionError::Eip2771Error("Invalid address length".to_string()));
        }

        let address = Address::from_slice(address_bytes);
        Ok(EvmAddress::from(address))
    }

    /// Parse executeWithProof function (custom implementation)
    fn parse_execute_with_proof_function(
        &self,
        data: &[u8],
    ) -> Result<EvmAddress, UserDetectionError> {
        // Similar to execute but with additional proof parameter
        // This would depend on the specific forwarder implementation
        self.parse_execute_function(data) // Simplified for now
    }

    /// Generic EIP-2771 parsing - try to extract address from first parameter
    fn parse_generic_eip2771(&self, data: &[u8]) -> Result<EvmAddress, UserDetectionError> {
        if data.len() < 36 {
            return Err(UserDetectionError::Eip2771Error(
                "Insufficient data for generic parsing".to_string(),
            ));
        }

        // Try to parse first parameter as address
        let potential_address = &data[4..36];
        let address_bytes = &potential_address[12..]; // Last 20 bytes

        if address_bytes.len() != 20 {
            return Err(UserDetectionError::Eip2771Error("Invalid address format".to_string()));
        }

        // Validate that this looks like a real address (not all zeros)
        if address_bytes.iter().all(|&b| b == 0) {
            return Err(UserDetectionError::Eip2771Error("Address is zero".to_string()));
        }

        let address = Address::from_slice(address_bytes);
        Ok(EvmAddress::from(address))
    }
}

impl Default for UserDetectionConfig {
    fn default() -> Self {
        Self {
            enable_header_detection: true,
            header_name: "X-End-User-Address".to_string(),
            enable_eip2771_parsing: true,
            trusted_forwarders: None,
            fallback_to_relayer: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use std::str::FromStr;

    #[test]
    fn test_header_detection() {
        let config = UserDetectionConfig::default();
        let detector = UserDetector::new(config);

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-End-User-Address",
            "0x742d35Cc6aF6C5C8c3C4B4C8e1A36F1c57F1b8Ff".parse().unwrap(),
        );

        let fallback = EvmAddress::from_str("0x0000000000000000000000000000000000000000").unwrap();
        let result = detector.detect_user(&headers, None, &[], &fallback).unwrap();

        assert_eq!(
            result.user_address,
            EvmAddress::from_str("0x742d35Cc6aF6C5C8c3C4B4C8e1A36F1c57F1b8Ff").unwrap()
        );
        assert!(matches!(result.detection_method, UserDetectionMethod::Header));
    }

    #[test]
    fn test_eip2771_execute_parsing() {
        let config = UserDetectionConfig::default();
        let detector = UserDetector::new(config);

        // Mock execute(address,bytes) calldata
        let mut calldata = vec![0x0e, 0x43, 0x31, 0x9d]; // execute function selector

        // Add first parameter (address) - 32 bytes with address in last 20 bytes
        calldata.extend_from_slice(&[0u8; 12]); // padding
        calldata.extend_from_slice(&[
            0x74, 0x2d, 0x35, 0xcc, 0x6a, 0xf6, 0xc5, 0xc8, 0xc3, 0xc4, 0xb4, 0xc8, 0xe1, 0xa3,
            0x6f, 0x1c, 0x57, 0xf1, 0xb8, 0xff,
        ]); // user address

        let user_address = detector.parse_execute_function(&calldata).unwrap();
        assert_eq!(
            user_address,
            EvmAddress::from_str("0x742d35Cc6aF6C5C8c3C4B4C8e1A36F1c57F1b8Ff").unwrap()
        );
    }

    #[test]
    fn test_fallback_detection() {
        let config = UserDetectionConfig {
            enable_header_detection: false,
            enable_eip2771_parsing: false,
            fallback_to_relayer: true,
            ..Default::default()
        };
        let detector = UserDetector::new(config);

        let headers = HeaderMap::new();
        let fallback = EvmAddress::from_str("0x1234567890123456789012345678901234567890").unwrap();

        let result = detector.detect_user(&headers, None, &[], &fallback).unwrap();

        assert_eq!(result.user_address, fallback);
        assert!(matches!(result.detection_method, UserDetectionMethod::Fallback));
    }
}
