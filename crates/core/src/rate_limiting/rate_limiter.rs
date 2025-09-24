use super::{
    detection::RateLimitDetector,
    types::{RateLimitDetectContext, RateLimitError, RateLimitOperation, RateLimitResult},
};
use crate::app_state::AppState;
use crate::relayer::get_relayer;
use crate::relayer::RelayerId;
use crate::shared::{not_found, too_many_requests, HttpError};
use crate::{
    common_types::EvmAddress,
    yaml::{RateLimitConfig, RateLimitWithInterval},
};
use axum::http::HeaderMap;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::error;

#[derive(Debug, Clone)]
struct UsageWindow {
    usage_count: u64,
    window_start: SystemTime,
}

pub struct RateLimiter {
    config: RateLimitConfig,
    detector: RateLimitDetector,
    usage_cache: Arc<RwLock<HashMap<String, UsageWindow>>>,
    global_usage_cache: Arc<RwLock<HashMap<String, UsageWindow>>>,
    relayer_usage_cache: Arc<RwLock<HashMap<String, UsageWindow>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let detector = RateLimitDetector::new(config.fallback_to_relayer);

        Self {
            config,
            detector,
            usage_cache: Arc::new(RwLock::new(HashMap::new())),
            global_usage_cache: Arc::new(RwLock::new(HashMap::new())),
            relayer_usage_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn parse_interval(interval: &str) -> u32 {
        match interval {
            "1m" => 60,
            _ => 60,
        }
    }

    fn get_limits_for_user(&self, _user_key: &str) -> (u64, u64, u32) {
        // Returns (transaction_limit, signing_limit, window_seconds)

        // First check for user-specific per-relayer limits
        if let Some(ref user_limits) = self.config.user_limits {
            if let Some(ref per_relayer) = user_limits.per_relayer {
                let window_seconds = Self::parse_interval(&per_relayer.interval);
                return (per_relayer.transactions, per_relayer.signing_operations, window_seconds);
            }
        }

        // Then check for general relayer limits (applies to all relayers)
        if let Some(ref relayer_limits) = self.config.relayer_limits {
            let window_seconds = Self::parse_interval(&relayer_limits.interval);
            return (
                relayer_limits.transactions,
                relayer_limits.signing_operations,
                window_seconds,
            );
        }

        // Default limits if none configured
        (100, 50, 60)
    }

    pub async fn check_and_reserve_rate_limit<'a>(
        state: &'a Arc<AppState>,
        headers: &HeaderMap,
        relayer_id: &RelayerId,
        operation: RateLimitOperation,
    ) -> Result<Option<RateLimitReservation<'a>>, HttpError> {
        let Some(ref rate_limiter) = state.user_rate_limiter else {
            return Ok(None);
        };

        let relayer = get_relayer(&state.db, &state.cache, relayer_id)
            .await?
            .ok_or(not_found("Relayer does not exist".to_string()))?;

        match rate_limiter.check_and_reserve(headers, &relayer.address, operation).await {
            Ok(reservation) => Ok(Some(reservation)),
            Err(RateLimitError::LimitExceeded { operation, current, limit, window_seconds }) => {
                error!(
                    "Rate limit exceeded: {}/{} {} in {}s",
                    current, limit, operation, window_seconds
                );
                Err(too_many_requests())
            }
            Err(RateLimitError::NoRateLimitKey) => {
                // do nothing it's ok to have no key
                Ok(None)
            }
            Err(e) => {
                error!("Rate limiting error: {}", e);
                // Don't block operation for rate limiting errors, just log
                Ok(None)
            }
        }
    }

    async fn check_and_reserve(
        &self,
        headers: &HeaderMap,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<RateLimitReservation, RateLimitError> {
        let context = self.detector.detect(headers, relayer_address)?;

        // First check relayer limits (ALL usage on this relayer, regardless of user)
        if let Some(ref relayer_limits) = self.config.relayer_limits {
            self.check_relayer_limits(relayer_address, operation, relayer_limits).await?;
        }

        // Then check user-specific limits
        let user_key = &context.key;

        // Check global user limits (all usage across all relayers for all users)
        if let Some(ref user_limits) = self.config.user_limits {
            if let Some(ref global_limits) = user_limits.global {
                self.check_global_user_limits(operation, global_limits).await?;
            }
        }

        // Check per-relayer user limits (user usage on this specific relayer)
        let result = self.check_user_limits(user_key, relayer_address, operation).await?;

        if !result.allowed {
            return Err(RateLimitError::LimitExceeded {
                operation: operation.as_str().to_string(),
                current: result.current_usage,
                limit: result.limit,
                window_seconds: result.window_seconds,
            });
        }

        // Reserve in all relevant caches
        self.reserve_user_operation(user_key, relayer_address, operation).await?;

        if let Some(ref user_limits) = self.config.user_limits {
            if user_limits.global.is_some() {
                self.reserve_global_user_operation(operation).await?;
            }
        }

        if self.config.relayer_limits.is_some() {
            self.reserve_relayer_operation(relayer_address, operation).await?;
        }

        Ok(RateLimitReservation {
            rate_limiter: self,
            user_key: user_key.clone(),
            operation,
            context,
            relayer_address: *relayer_address,
            reserved: true,
        })
    }

    async fn check_relayer_limits(
        &self,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
        relayer_limits: &RateLimitWithInterval,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();
        let window_seconds = Self::parse_interval(&relayer_limits.interval);

        let (limit, cache_key) = match operation {
            RateLimitOperation::Transaction => {
                (relayer_limits.transactions, format!("relayer_{}_{}", relayer_address, "tx"))
            }
            RateLimitOperation::Signing => (
                relayer_limits.signing_operations,
                format!("relayer_{}_{}", relayer_address, "signing"),
            ),
        };

        let window_start = self.calculate_window_start(current_time, window_seconds);

        let relayer_cache = self.relayer_usage_cache.read().await;
        let usage = relayer_cache
            .get(&cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let current_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() > window_seconds as u64)
            .unwrap_or(true)
        {
            1 // Window expired, start fresh
        } else {
            usage.usage_count + 1
        };

        if current_usage > limit {
            return Err(RateLimitError::LimitExceeded {
                operation: format!("relayer_{}", operation.as_str()),
                current: current_usage,
                limit,
                window_seconds,
            });
        }

        Ok(())
    }

    async fn check_global_user_limits(
        &self,
        operation: RateLimitOperation,
        global_limits: &RateLimitWithInterval,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();
        let window_seconds = Self::parse_interval(&global_limits.interval);

        let (limit, cache_key) = match operation {
            RateLimitOperation::Transaction => (global_limits.transactions, "global_user_tx"),
            RateLimitOperation::Signing => {
                (global_limits.signing_operations, "global_user_signing")
            }
        };

        let window_start = self.calculate_window_start(current_time, window_seconds);

        let global_cache = self.global_usage_cache.read().await;
        let usage = global_cache
            .get(cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let current_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() > window_seconds as u64)
            .unwrap_or(true)
        {
            1 // Window expired, start fresh
        } else {
            usage.usage_count + 1
        };

        if current_usage > limit {
            return Err(RateLimitError::LimitExceeded {
                operation: cache_key.to_string(),
                current: current_usage,
                limit,
                window_seconds,
            });
        }

        Ok(())
    }

    async fn check_user_limits(
        &self,
        user_key: &str,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<RateLimitResult, RateLimitError> {
        let (tx_limit, signing_limit, window_seconds) = self.get_limits_for_user(user_key);

        let current_time = SystemTime::now();

        let limit = match operation {
            RateLimitOperation::Transaction => tx_limit,
            RateLimitOperation::Signing => signing_limit,
        };

        if limit == u64::MAX {
            return Ok(RateLimitResult {
                allowed: true,
                current_usage: 0,
                limit: u64::MAX,
                window_seconds,
                reset_time: current_time,
            });
        }

        let cache_key = format!("{}_{}_{}", user_key, relayer_address, operation.as_str());
        let window_start = self.calculate_window_start(current_time, window_seconds);

        let usage_cache = self.usage_cache.read().await;
        let usage = usage_cache
            .get(&cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let current_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() > window_seconds as u64)
            .unwrap_or(true)
        {
            1 // Window expired, start fresh
        } else {
            usage.usage_count + 1
        };

        let allowed = current_usage <= limit;
        let reset_time = window_start + std::time::Duration::from_secs(window_seconds as u64);

        Ok(RateLimitResult { allowed, current_usage, limit, window_seconds, reset_time })
    }

    async fn reserve_user_operation(
        &self,
        user_key: &str,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let cache_key = format!("{}_{}_{}", user_key, relayer_address, operation.as_str());
        let current_time = SystemTime::now();
        let (_, _, window_seconds) = self.get_limits_for_user(user_key);
        let window_start = self.calculate_window_start(current_time, window_seconds);

        let mut usage_cache = self.usage_cache.write().await;
        let usage = usage_cache
            .get(&cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let new_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() > window_seconds as u64)
            .unwrap_or(true)
        {
            UsageWindow { usage_count: 1, window_start }
        } else {
            UsageWindow { usage_count: usage.usage_count + 1, window_start: usage.window_start }
        };

        usage_cache.insert(cache_key, new_usage);
        Ok(())
    }

    async fn reserve_global_user_operation(
        &self,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();
        let mut global_cache = self.global_usage_cache.write().await;

        if let Some(ref user_limits) = self.config.user_limits {
            if let Some(ref global_limits) = user_limits.global {
                let window_seconds = Self::parse_interval(&global_limits.interval);
                let cache_key = match operation {
                    RateLimitOperation::Transaction => "global_user_tx",
                    RateLimitOperation::Signing => "global_user_signing",
                };

                let window_start = self.calculate_window_start(current_time, window_seconds);

                let usage = global_cache
                    .get(cache_key)
                    .cloned()
                    .unwrap_or(UsageWindow { usage_count: 0, window_start });

                let new_usage = if current_time
                    .duration_since(usage.window_start)
                    .map(|d| d.as_secs() > window_seconds as u64)
                    .unwrap_or(true)
                {
                    UsageWindow { usage_count: 1, window_start }
                } else {
                    UsageWindow {
                        usage_count: usage.usage_count + 1,
                        window_start: usage.window_start,
                    }
                };

                global_cache.insert(cache_key.to_string(), new_usage);
            }
        }

        Ok(())
    }

    async fn reserve_relayer_operation(
        &self,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();

        if let Some(ref relayer_limits) = self.config.relayer_limits {
            let window_seconds = Self::parse_interval(&relayer_limits.interval);
            let cache_key = match operation {
                RateLimitOperation::Transaction => format!("relayer_{}_{}", relayer_address, "tx"),
                RateLimitOperation::Signing => format!("relayer_{}_{}", relayer_address, "signing"),
            };

            let window_start = self.calculate_window_start(current_time, window_seconds);
            let mut relayer_cache = self.relayer_usage_cache.write().await;

            let usage = relayer_cache
                .get(&cache_key)
                .cloned()
                .unwrap_or(UsageWindow { usage_count: 0, window_start });

            let new_usage = if current_time
                .duration_since(usage.window_start)
                .map(|d| d.as_secs() > window_seconds as u64)
                .unwrap_or(true)
            {
                UsageWindow { usage_count: 1, window_start }
            } else {
                UsageWindow { usage_count: usage.usage_count + 1, window_start: usage.window_start }
            };

            relayer_cache.insert(cache_key, new_usage);
        }

        Ok(())
    }

    async fn revert_user_operation(
        &self,
        user_key: &str,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let cache_key = format!("{}_{}_{}", user_key, relayer_address, operation.as_str());

        let mut usage_cache = self.usage_cache.write().await;
        if let Some(mut usage) = usage_cache.get(&cache_key).cloned() {
            if usage.usage_count > 0 {
                usage.usage_count -= 1;
                usage_cache.insert(cache_key, usage);
            }
        }

        Ok(())
    }

    async fn revert_global_user_operation(
        &self,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let mut global_cache = self.global_usage_cache.write().await;

        let cache_key = match operation {
            RateLimitOperation::Transaction => "global_user_tx",
            RateLimitOperation::Signing => "global_user_signing",
        };

        if let Some(mut usage) = global_cache.get(cache_key).cloned() {
            if usage.usage_count > 0 {
                usage.usage_count -= 1;
                global_cache.insert(cache_key.to_string(), usage);
            }
        }

        Ok(())
    }

    async fn revert_relayer_operation(
        &self,
        relayer_address: &EvmAddress,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let cache_key = match operation {
            RateLimitOperation::Transaction => format!("relayer_{}_{}", relayer_address, "tx"),
            RateLimitOperation::Signing => format!("relayer_{}_{}", relayer_address, "signing"),
        };

        let mut relayer_cache = self.relayer_usage_cache.write().await;
        if let Some(mut usage) = relayer_cache.get(&cache_key).cloned() {
            if usage.usage_count > 0 {
                usage.usage_count -= 1;
                relayer_cache.insert(cache_key, usage);
            }
        }

        Ok(())
    }

    fn calculate_window_start(&self, current_time: SystemTime, window_seconds: u32) -> SystemTime {
        let current_timestamp = current_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let window_start_timestamp =
            (current_timestamp / window_seconds as u64) * window_seconds as u64;
        UNIX_EPOCH + std::time::Duration::from_secs(window_start_timestamp)
    }
}

pub struct RateLimitReservation<'a> {
    rate_limiter: &'a RateLimiter,
    user_key: String,
    operation: RateLimitOperation,
    context: RateLimitDetectContext,
    relayer_address: EvmAddress,
    reserved: bool,
}

impl<'a> RateLimitReservation<'a> {
    pub fn commit(mut self) {
        self.reserved = false;
    }

    pub async fn revert(mut self) -> Result<(), RateLimitError> {
        if self.reserved {
            // Revert user operation
            self.rate_limiter.revert_user_operation(&self.user_key, &self.relayer_address, self.operation).await?;

            // Revert global user operation if applicable
            if let Some(ref user_limits) = self.rate_limiter.config.user_limits {
                if user_limits.global.is_some() {
                    self.rate_limiter.revert_global_user_operation(self.operation).await?;
                }
            }

            // Revert relayer operation if applicable
            if self.rate_limiter.config.relayer_limits.is_some() {
                self.rate_limiter
                    .revert_relayer_operation(&self.relayer_address, self.operation)
                    .await?;
            }

            self.reserved = false;
        }
        Ok(())
    }
}

impl<'a> Drop for RateLimitReservation<'a> {
    fn drop(&mut self) {
        if self.reserved {
            let rate_limiter = self.rate_limiter.clone();
            let user_key = self.user_key.clone();
            let operation = self.operation;
            let relayer_address = self.relayer_address;
            let config = self.rate_limiter.config.clone();
            tokio::spawn(async move {
                // Revert user operation
                let _ = rate_limiter.revert_user_operation(&user_key, &relayer_address, operation).await;

                // Revert global user operation if applicable
                if let Some(ref user_limits) = config.user_limits {
                    if user_limits.global.is_some() {
                        let _ = rate_limiter.revert_global_user_operation(operation).await;
                    }
                }

                // Revert relayer operation if applicable
                if config.relayer_limits.is_some() {
                    let _ =
                        rate_limiter.revert_relayer_operation(&relayer_address, operation).await;
                }
            });
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            detector: RateLimitDetector::new(self.config.fallback_to_relayer),
            usage_cache: self.usage_cache.clone(),
            global_usage_cache: self.global_usage_cache.clone(),
            relayer_usage_cache: self.relayer_usage_cache.clone(),
        }
    }
}
