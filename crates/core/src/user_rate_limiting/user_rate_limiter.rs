use crate::common_types::EvmAddress;
use crate::postgres::{PostgresClient, PostgresError};
use crate::yaml::RateLimitConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UserRateLimitError {
    #[error("Database error: {0}")]
    Database(#[from] PostgresError),

    #[error("Rate limit exceeded for {rule_type}: {current}/{limit} per {window_seconds}s")]
    LimitExceeded { rule_type: String, current: u64, limit: u64, window_seconds: u32 },

    #[error("Configuration error: {0}")]
    Configuration(String),
}

#[derive(Debug, Clone)]
pub struct UserRateLimitCheck {
    pub allowed: bool,
    pub rule_type: String,
    pub current_usage: u64,
    pub limit: u64,
    pub window_seconds: u32,
    pub reset_time: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRule {
    pub user_identifier: String,
    pub rule_type: String,
    pub limit_value: u64,
    pub window_duration_seconds: u32,
    pub is_unlimited: bool,
}

#[derive(Debug, Clone)]
struct UsageWindow {
    usage_count: u64,
    window_start: SystemTime,
    last_request_at: SystemTime,
}

/// Hybrid rate limiter with database persistence and in-memory caching
pub struct UserRateLimiter {
    config: RateLimitConfig,
    db: Arc<PostgresClient>,
    in_memory_cache: Arc<RwLock<HashMap<String, HashMap<String, UsageWindow>>>>,
    rules_cache: Arc<RwLock<HashMap<String, HashMap<String, RateLimitRule>>>>,
}

impl UserRateLimiter {
    pub fn new(config: RateLimitConfig, db: Arc<PostgresClient>) -> Self {
        Self {
            config,
            db,
            in_memory_cache: Arc::new(RwLock::new(HashMap::new())),
            rules_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize rate limiter by loading rules from database and config
    pub async fn initialize(&self) -> Result<(), UserRateLimitError> {
        // Load existing rules from database
        self.load_rules_from_database().await?;

        // Apply default rate limits from config
        self.apply_default_rate_limits().await?;

        // Apply user-specific overrides from config
        self.apply_user_rate_limit_overrides().await?;

        Ok(())
    }

    /// Check if a request should be allowed based on rate limits
    pub async fn check_rate_limit(
        &self,
        user_identifier: &str,
        rule_type: &str,
        increment: u64,
    ) -> Result<UserRateLimitCheck, UserRateLimitError> {
        let rule = self.get_rate_limit_rule(user_identifier, rule_type).await?;

        if rule.is_unlimited {
            return Ok(UserRateLimitCheck {
                allowed: true,
                rule_type: rule_type.to_string(),
                current_usage: 0,
                limit: u64::MAX,
                window_seconds: rule.window_duration_seconds,
                reset_time: SystemTime::now(),
            });
        }

        let current_time = SystemTime::now();
        let window_start = self.calculate_window_start(current_time, rule.window_duration_seconds);

        // Check in-memory cache first
        let mut cache = self.in_memory_cache.write().await;
        let user_cache = cache.entry(user_identifier.to_string()).or_insert_with(HashMap::new);

        let usage = user_cache.get(rule_type).cloned().unwrap_or(UsageWindow {
            usage_count: 0,
            window_start,
            last_request_at: current_time,
        });

        // If window has expired, reset usage
        let (current_usage, effective_window_start) = if current_time
            .duration_since(usage.window_start)
            .map(|d| d.as_secs() as u32 > rule.window_duration_seconds)
            .unwrap_or(true)
        {
            // Window expired, start fresh
            (increment, window_start)
        } else {
            // Same window, add to existing usage
            (usage.usage_count + increment, usage.window_start)
        };

        let allowed = current_usage <= rule.limit_value;

        if allowed {
            // Update in-memory cache
            user_cache.insert(
                rule_type.to_string(),
                UsageWindow {
                    usage_count: current_usage,
                    window_start: effective_window_start,
                    last_request_at: current_time,
                },
            );

            // Asynchronously persist to database
            let db = self.db.clone();
            let user_id = user_identifier.to_string();
            let rule_type_str = rule_type.to_string();
            tokio::spawn(async move {
                let _ = Self::persist_usage_to_db(
                    &db,
                    &user_id,
                    &rule_type_str,
                    effective_window_start,
                    current_usage,
                )
                .await;
            });
        }

        let reset_time = effective_window_start
            + std::time::Duration::from_secs(rule.window_duration_seconds as u64);

        Ok(UserRateLimitCheck {
            allowed,
            rule_type: rule_type.to_string(),
            current_usage,
            limit: rule.limit_value,
            window_seconds: rule.window_duration_seconds,
            reset_time,
        })
    }

    /// Record transaction metadata for analytics
    pub async fn record_transaction_metadata(
        &self,
        transaction_hash: Option<&str>,
        relayer_id: &Uuid,
        end_user_address: &EvmAddress,
        detection_method: &str,
        transaction_type: &str,
        gas_used: Option<u64>,
        rate_limits_applied: &[String],
    ) -> Result<(), UserRateLimitError> {
        let rate_limits_json = serde_json::to_string(rate_limits_applied).map_err(|e| {
            UserRateLimitError::Configuration(format!("Failed to serialize rate limits: {}", e))
        })?;

        self.db.execute(
            "INSERT INTO transaction_rate_limit_metadata 
             (transaction_hash, relayer_id, end_user_address, detection_method, transaction_type, gas_used, rate_limits_applied)
             VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb)",
            &[
                &transaction_hash,
                relayer_id,
                &format!("{:?}", end_user_address),
                &detection_method,
                &transaction_type,
                &(gas_used.map(|g| g as i64)),
                &rate_limits_json,
            ],
        ).await?;

        Ok(())
    }

    async fn get_rate_limit_rule(
        &self,
        user_identifier: &str,
        rule_type: &str,
    ) -> Result<RateLimitRule, UserRateLimitError> {
        let rules_cache = self.rules_cache.read().await;

        // Check user-specific rules first
        if let Some(user_rules) = rules_cache.get(user_identifier) {
            if let Some(rule) = user_rules.get(rule_type) {
                return Ok(rule.clone());
            }
        }

        // Fall back to default rules
        if let Some(default_rules) = rules_cache.get("_default") {
            if let Some(rule) = default_rules.get(rule_type) {
                return Ok(rule.clone());
            }
        }

        Err(UserRateLimitError::Configuration(format!(
            "No rate limit rule found for type: {}",
            rule_type
        )))
    }

    async fn load_rules_from_database(&self) -> Result<(), UserRateLimitError> {
        let rows = self.db.query(
            "SELECT user_identifier, rule_type, limit_value, window_duration_seconds, is_unlimited FROM rate_limit_rules",
            &[],
        ).await?;

        let mut rules_cache = self.rules_cache.write().await;
        for row in rows {
            let user_identifier: String = row.get("user_identifier");
            let rule_type: String = row.get("rule_type");
            let limit_value: i64 = row.get("limit_value");
            let window_duration_seconds: i32 = row.get("window_duration_seconds");
            let is_unlimited: bool = row.get("is_unlimited");

            let rule = RateLimitRule {
                user_identifier: user_identifier.clone(),
                rule_type: rule_type.clone(),
                limit_value: limit_value as u64,
                window_duration_seconds: window_duration_seconds as u32,
                is_unlimited,
            };

            rules_cache.entry(user_identifier).or_insert_with(HashMap::new).insert(rule_type, rule);
        }

        Ok(())
    }

    async fn apply_default_rate_limits(&self) -> Result<(), UserRateLimitError> {
        let defaults = &self.config.default_rules;

        if let Some(tx_limit) = defaults.transactions_per_minute {
            self.upsert_rate_limit_rule("_default", "transactions_per_minute", tx_limit, 60, false)
                .await?;
        }

        if let Some(tx_hour_limit) = defaults.transactions_per_hour {
            self.upsert_rate_limit_rule(
                "_default",
                "transactions_per_hour",
                tx_hour_limit,
                3600,
                false,
            )
            .await?;
        }

        if let Some(gas_limit) = defaults.gas_per_minute {
            self.upsert_rate_limit_rule("_default", "gas_per_minute", gas_limit, 60, false).await?;
        }

        if let Some(signing_limit) = defaults.signing_operations_per_minute {
            self.upsert_rate_limit_rule(
                "_default",
                "signing_operations_per_minute",
                signing_limit,
                60,
                false,
            )
            .await?;
        }

        if let Some(concurrent_limit) = defaults.concurrent_transactions {
            self.upsert_rate_limit_rule(
                "_default",
                "concurrent_transactions",
                concurrent_limit,
                0,
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn apply_user_rate_limit_overrides(&self) -> Result<(), UserRateLimitError> {
        if let Some(ref user_overrides) = self.config.user_overrides {
            for (user_address, limits) in user_overrides {
                let user_id = user_address.clone();

                if let Some(tx_limit) = limits.transactions_per_minute {
                    self.upsert_rate_limit_rule(
                        &user_id,
                        "transactions_per_minute",
                        tx_limit,
                        60,
                        false,
                    )
                    .await?;
                }

                if let Some(tx_hour_limit) = limits.transactions_per_hour {
                    self.upsert_rate_limit_rule(
                        &user_id,
                        "transactions_per_hour",
                        tx_hour_limit,
                        3600,
                        false,
                    )
                    .await?;
                }

                if let Some(gas_limit) = limits.gas_per_minute {
                    self.upsert_rate_limit_rule(&user_id, "gas_per_minute", gas_limit, 60, false)
                        .await?;
                }

                if let Some(signing_limit) = limits.signing_operations_per_minute {
                    self.upsert_rate_limit_rule(
                        &user_id,
                        "signing_operations_per_minute",
                        signing_limit,
                        60,
                        false,
                    )
                    .await?;
                }

                if let Some(concurrent_limit) = limits.concurrent_transactions {
                    self.upsert_rate_limit_rule(
                        &user_id,
                        "concurrent_transactions",
                        concurrent_limit,
                        0,
                        false,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn upsert_rate_limit_rule(
        &self,
        user_identifier: &str,
        rule_type: &str,
        limit_value: u64,
        window_duration_seconds: u32,
        is_unlimited: bool,
    ) -> Result<(), UserRateLimitError> {
        // Update database
        self.db.execute(
            "INSERT INTO rate_limit_rules (user_identifier, rule_type, limit_value, window_duration_seconds, is_unlimited, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (user_identifier, rule_type)
             DO UPDATE SET limit_value = $3, window_duration_seconds = $4, is_unlimited = $5, updated_at = NOW()",
            &[
                &user_identifier,
                &rule_type,
                &(limit_value as i64),
                &(window_duration_seconds as i32),
                &is_unlimited,
            ],
        ).await?;

        // Update cache
        let rule = RateLimitRule {
            user_identifier: user_identifier.to_string(),
            rule_type: rule_type.to_string(),
            limit_value,
            window_duration_seconds,
            is_unlimited,
        };

        let mut rules_cache = self.rules_cache.write().await;
        rules_cache
            .entry(user_identifier.to_string())
            .or_insert_with(HashMap::new)
            .insert(rule_type.to_string(), rule);

        Ok(())
    }

    fn calculate_window_start(&self, current_time: SystemTime, window_seconds: u32) -> SystemTime {
        let current_timestamp = current_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let window_start_timestamp =
            (current_timestamp / window_seconds as u64) * window_seconds as u64;
        UNIX_EPOCH + std::time::Duration::from_secs(window_start_timestamp)
    }

    async fn persist_usage_to_db(
        db: &PostgresClient,
        user_identifier: &str,
        rule_type: &str,
        window_start: SystemTime,
        usage_count: u64,
    ) -> Result<(), PostgresError> {
        let window_start_timestamp =
            window_start.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let window_start_datetime =
            chrono::DateTime::from_timestamp(window_start_timestamp, 0).unwrap();

        db.execute(
            "INSERT INTO rate_limit_usage (user_identifier, rule_type, window_start, usage_count, last_request_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (user_identifier, rule_type, window_start)
             DO UPDATE SET usage_count = EXCLUDED.usage_count, last_request_at = NOW()",
            &[
                &user_identifier,
                &rule_type,
                &window_start_datetime,
                &(usage_count as i64),
            ],
        ).await?;

        Ok(())
    }

    /// Clean up old usage records (called periodically)
    pub async fn cleanup_old_usage(&self) -> Result<(), UserRateLimitError> {
        self.db.execute("SELECT cleanup_old_rate_limit_usage()", &[]).await?;
        Ok(())
    }
}
