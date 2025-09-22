use super::{
    detection::RateLimitDetector,
    types::{RateLimitDetectContext, RateLimitError, RateLimitOperation, RateLimitResult},
};
use crate::app_state::AppState;
use crate::relayer::get_relayer;
use crate::relayer::RelayerId;
use crate::{
    common_types::EvmAddress,
    yaml::{RateLimitConfig, RateLimits},
    GlobalRateLimits,
};
use axum::http::{HeaderMap, StatusCode};
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
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        let detector = RateLimitDetector::new(config.fallback_to_relayer);

        Self {
            config,
            detector,
            usage_cache: Arc::new(RwLock::new(HashMap::new())),
            global_usage_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn check_and_reserve_rate_limit<'a>(
        state: &'a Arc<AppState>,
        headers: &HeaderMap,
        relayer_id: &RelayerId,
        operation: RateLimitOperation,
    ) -> Result<Option<RateLimitReservation<'a>>, StatusCode> {
        let Some(ref rate_limiter) = state.user_rate_limiter else {
            return Ok(None);
        };

        let relayer = get_relayer(&state.db, &state.cache, relayer_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        match rate_limiter.check_and_reserve(headers, &relayer.address, operation).await {
            Ok(reservation) => Ok(Some(reservation)),
            Err(RateLimitError::LimitExceeded { operation, current, limit, window_seconds }) => {
                error!(
                    "Rate limit exceeded: {}/{} {} in {}s",
                    current, limit, operation, window_seconds
                );
                Err(StatusCode::TOO_MANY_REQUESTS)
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

        if let Some(ref global_limits) = self.config.global_limits {
            self.check_global_limits(operation, global_limits).await?;
        }

        let user_key = &context.key;
        let result = self.check_user_limits(user_key, operation).await?;

        if !result.allowed {
            return Err(RateLimitError::LimitExceeded {
                operation: operation.as_str().to_string(),
                current: result.current_usage,
                limit: result.limit,
                window_seconds: result.window_seconds,
            });
        }

        self.reserve_operation(user_key, operation).await?;
        if self.config.global_limits.is_some() {
            self.reserve_global_operation(operation).await?;
        }

        Ok(RateLimitReservation {
            rate_limiter: self,
            user_key: user_key.clone(),
            operation,
            context,
            reserved: true,
        })
    }

    async fn check_global_limits(
        &self,
        operation: RateLimitOperation,
        global_limits: &GlobalRateLimits,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();

        let limits_to_check = match operation {
            RateLimitOperation::Transaction => {
                let mut checks = Vec::new();
                if let Some(limit) = global_limits.max_transactions_per_minute {
                    checks.push(("global_tx_per_minute", limit, 60));
                }
                checks
            }
            RateLimitOperation::Signing => {
                let mut checks = Vec::new();
                if let Some(limit) = global_limits.max_signing_operations_per_minute {
                    checks.push(("global_signing_per_minute", limit, 60));
                }
                checks
            }
        };

        let mut global_cache = self.global_usage_cache.write().await;

        for (cache_key, limit, window_seconds) in limits_to_check {
            let window_start = self.calculate_window_start(current_time, window_seconds);

            let usage = global_cache
                .get(cache_key)
                .cloned()
                .unwrap_or(UsageWindow { usage_count: 0, window_start });

            let current_usage = if current_time
                .duration_since(usage.window_start)
                .map(|d| d.as_secs() as u32 > window_seconds)
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
        }

        Ok(())
    }

    async fn check_user_limits(
        &self,
        user_key: &str,
        operation: RateLimitOperation,
    ) -> Result<RateLimitResult, RateLimitError> {
        let limits = self.get_limits_for_user(user_key);

        let current_time = SystemTime::now();

        // Check per-minute limits
        let (limit, window_seconds) = match operation {
            RateLimitOperation::Transaction => {
                (limits.transactions_per_minute.unwrap_or(u64::MAX), 60)
            }
            RateLimitOperation::Signing => {
                (limits.signing_operations_per_minute.unwrap_or(u64::MAX), 60)
            }
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

        let cache_key = format!("{}_{}", user_key, operation.as_str());
        let window_start = self.calculate_window_start(current_time, window_seconds);

        let usage_cache = self.usage_cache.read().await;
        let usage = usage_cache
            .get(&cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let current_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() as u32 > window_seconds)
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

    async fn reserve_operation(
        &self,
        user_key: &str,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let cache_key = format!("{}_{}", user_key, operation.as_str());
        let current_time = SystemTime::now();
        let window_start = self.calculate_window_start(current_time, 60);

        let mut usage_cache = self.usage_cache.write().await;
        let usage = usage_cache
            .get(&cache_key)
            .cloned()
            .unwrap_or(UsageWindow { usage_count: 0, window_start });

        let new_usage = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() as u32 > 60)
            .unwrap_or(true)
        {
            UsageWindow { usage_count: 1, window_start }
        } else {
            UsageWindow { usage_count: usage.usage_count + 1, window_start: usage.window_start }
        };

        usage_cache.insert(cache_key, new_usage);
        Ok(())
    }

    async fn reserve_global_operation(
        &self,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let current_time = SystemTime::now();
        let mut global_cache = self.global_usage_cache.write().await;

        let cache_keys = match operation {
            RateLimitOperation::Transaction => {
                vec![("global_tx_per_minute", 60)]
            }
            RateLimitOperation::Signing => {
                vec![("global_signing_per_minute", 60)]
            }
        };

        for (cache_key, window_seconds) in cache_keys {
            let window_start = self.calculate_window_start(current_time, window_seconds);

            let usage = global_cache
                .get(cache_key)
                .cloned()
                .unwrap_or(UsageWindow { usage_count: 0, window_start });

            let new_usage = if current_time
                .duration_since(usage.window_start)
                .map(|d| d.as_secs() as u32 > window_seconds)
                .unwrap_or(true)
            {
                UsageWindow { usage_count: 1, window_start }
            } else {
                UsageWindow { usage_count: usage.usage_count + 1, window_start: usage.window_start }
            };

            global_cache.insert(cache_key.to_string(), new_usage);
        }

        Ok(())
    }

    async fn revert_operation(
        &self,
        user_key: &str,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let cache_key = format!("{}_{}", user_key, operation.as_str());

        let mut usage_cache = self.usage_cache.write().await;
        if let Some(mut usage) = usage_cache.get(&cache_key).cloned() {
            if usage.usage_count > 0 {
                usage.usage_count -= 1;
                usage_cache.insert(cache_key, usage);
            }
        }

        // Also revert global if needed
        if self.config.global_limits.is_some() {
            self.revert_global_operation(operation).await?;
        }

        Ok(())
    }

    async fn revert_global_operation(
        &self,
        operation: RateLimitOperation,
    ) -> Result<(), RateLimitError> {
        let mut global_cache = self.global_usage_cache.write().await;

        let cache_keys = match operation {
            RateLimitOperation::Transaction => vec!["global_tx_per_minute"],
            RateLimitOperation::Signing => {
                vec!["global_signing_per_minute"]
            }
        };

        for cache_key in cache_keys {
            if let Some(mut usage) = global_cache.get(cache_key).cloned() {
                if usage.usage_count > 0 {
                    usage.usage_count -= 1;
                    global_cache.insert(cache_key.to_string(), usage);
                }
            }
        }

        Ok(())
    }

    fn get_limits_for_user(&self, user_key: &str) -> RateLimits {
        if let Some(ref unlimited_users) = self.config.user_unlimited_overrides {
            if unlimited_users.contains(&user_key.to_string()) {
                return RateLimits { ..Default::default() };
            }
        }

        self.config.limits.clone().unwrap_or_default()
    }

    fn calculate_window_start(&self, current_time: SystemTime, window_seconds: u32) -> SystemTime {
        let current_timestamp = current_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let window_start_timestamp =
            (current_timestamp / window_seconds as u64) * window_seconds as u64;
        UNIX_EPOCH + std::time::Duration::from_secs(window_start_timestamp)
    }
}

impl Default for RateLimits {
    fn default() -> Self {
        Self { transactions_per_minute: Some(100), signing_operations_per_minute: Some(50) }
    }
}

pub struct RateLimitReservation<'a> {
    rate_limiter: &'a RateLimiter,
    user_key: String,
    operation: RateLimitOperation,
    context: RateLimitDetectContext,
    reserved: bool,
}

impl<'a> RateLimitReservation<'a> {
    pub fn commit(mut self) {
        self.reserved = false;
    }

    pub async fn revert(mut self) -> Result<(), RateLimitError> {
        if self.reserved {
            self.rate_limiter.revert_operation(&self.user_key, self.operation).await?;
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
            tokio::spawn(async move {
                let _ = rate_limiter.revert_operation(&user_key, operation).await;
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
        }
    }
}
