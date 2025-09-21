use crate::{
    app_state::AppState,
    relayer::{get_relayer, types::RelayerId},
};
use axum::http::{HeaderMap, StatusCode};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use super::{RateLimitError, RateLimitOperation, RateLimitReservation, RateLimiter};

/// Helper function to check and reserve rate limits for any operation.
///
/// This eliminates the boilerplate code that was repeated across all endpoints.
///
/// # Arguments
/// * `state` - The application state containing the rate limiter
/// * `headers` - HTTP headers for rate limit key detection
/// * `relayer_id` - The relayer ID to get the address for rate limiting
/// * `operation` - The type of operation (Transaction or Signing)
///
/// # Returns
/// * `Ok(Option<RateLimitReservation>)` - Reservation if rate limiting is enabled
/// * `Err(StatusCode)` - If rate limit exceeded or other errors
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
