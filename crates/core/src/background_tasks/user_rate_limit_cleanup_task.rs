use crate::rrelayer_info;
use std::sync::Arc;
use tracing::info;

/// Placeholder for rate limiter cleanup task.
///
/// The new rate limiter uses in-memory caching with time-based windows,
/// so no database cleanup is needed.
///
/// # Arguments
/// * `_user_rate_limiter` - The user rate limiter instance (unused)
pub async fn run_user_rate_limit_cleanup_task(
    _user_rate_limiter: Arc<crate::rate_limiting::RateLimiter>,
) {
    rrelayer_info!("Rate limiter cleanup not needed - using in-memory cache");
    info!("Rate limiter cleanup task placeholder initialized");
}
