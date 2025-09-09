use crate::{user_rate_limiting::UserRateLimiter, rrelayer_error, rrelayer_info};
use std::{sync::Arc, time::Duration};
use tracing::{error, info};

/// Runs the rate limiter cleanup background task.
///
/// This function starts a periodic cleanup task that removes old rate limit
/// usage records from the database to prevent unbounded growth.
///
/// # Arguments
/// * `user_rate_limiter` - The user rate limiter instance to run cleanup on
pub async fn run_user_rate_limit_cleanup_task(user_rate_limiter: Arc<UserRateLimiter>) {
    rrelayer_info!("Starting rate limiter cleanup task");

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Every hour
        loop {
            interval.tick().await;
            if let Err(e) = user_rate_limiter.cleanup_old_usage().await {
                error!("Rate limiter cleanup error: {}", e);
            }
        }
    });

    info!("Rate limiter cleanup task initialized");
}
