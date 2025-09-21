use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Simple enum for rate limiting operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitOperation {
    Transaction,
    Signing,
}

impl RateLimitOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            RateLimitOperation::Transaction => "transactions",
            RateLimitOperation::Signing => "signing",
        }
    }
}

/// User detection method for rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitDetectMethod {
    Header,
    Fallback,
}

/// Transaction type classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Direct,
    Gasless,
    Automated,
}

/// Context for rate limit detection
#[derive(Debug, Clone)]
pub struct RateLimitDetectContext {
    pub key: String,
    pub detection_method: RateLimitDetectMethod,
    pub transaction_type: TransactionType,
}

/// Rate limiting errors
#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for {operation}: {current}/{limit} in {window_seconds}s")]
    LimitExceeded { operation: String, current: u64, limit: u64, window_seconds: u32 },

    #[error("Database error: {0}")]
    Database(#[from] crate::postgres::PostgresError),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("No rate limit key")]
    NoRateLimitKey,
}

/// Simple result type for rate limit checks
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub current_usage: u64,
    pub limit: u64,
    pub window_seconds: u32,
    pub reset_time: std::time::SystemTime,
}
