use crate::postgres::PostgresError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitDetectMethod {
    Header,
    Fallback,
}

#[derive(Debug, Clone)]
pub struct RateLimitDetectContext {
    pub key: String,
}

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for {operation}: {current}/{limit} in {window_seconds}s")]
    LimitExceeded { operation: String, current: u64, limit: u64, window_seconds: u32 },

    #[error("Database error: {0}")]
    Database(#[from] PostgresError),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("No rate limit key")]
    NoRateLimitKey,
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub current_usage: u64,
    pub limit: u64,
    pub window_seconds: u32,
    #[allow(dead_code)]
    pub reset_time: std::time::SystemTime,
}
