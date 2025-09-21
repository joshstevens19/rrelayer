use crate::{
    postgres::PostgresClient,
    rrelayer_error, rrelayer_info,
    webhooks::db::write::{
        CreateWebhookDeliveryRequest, UpdateWebhookDeliveryRequest, WebhookDeliveryStatus,
    },
};
use chrono::Utc;
use reqwest::{Client, Response};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::{debug, warn};

use super::types::{WebhookDelivery, WebhookDeliveryConfig};

/// HTTP client for sending webhook requests with retry logic.
///
/// Handles the actual HTTP delivery of webhook payloads including:
/// - Request signing with HMAC-SHA256
/// - Timeout management
/// - Retry logic with exponential backoff
/// - Concurrent delivery processing
/// - Database logging of delivery attempts and outcomes
pub struct WebhookSender {
    client: Client,
    config: WebhookDeliveryConfig,
    pub db: Arc<PostgresClient>,
}

impl WebhookSender {
    /// Creates a new webhook sender with the specified configuration.
    ///
    /// # Arguments
    /// * `config` - Delivery configuration including timeouts and retry settings
    /// * `db` - Database client for logging webhook delivery attempts
    ///
    /// # Returns
    /// * `Ok(WebhookSender)` - Configured webhook sender
    /// * `Err(reqwest::Error)` - If HTTP client creation fails
    pub fn new(
        config: WebhookDeliveryConfig,
        db: Arc<PostgresClient>,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds as u64))
            .user_agent("RRelayer-Webhooks/1.0")
            .build()?;

        Ok(Self { client, config, db })
    }

    /// Sends a webhook with automatic retry logic.
    ///
    /// Attempts to deliver the webhook and updates the delivery status based on
    /// the response. On failure, schedules retries with exponential backoff.
    /// Logs all attempts and outcomes to the database for monitoring and debugging.
    ///
    /// # Arguments
    /// * `delivery` - The webhook delivery to process
    ///
    /// # Returns
    /// * `WebhookDelivery` - Updated delivery with new status and retry information
    pub async fn send_webhook(&self, mut delivery: WebhookDelivery) -> WebhookDelivery {
        rrelayer_info!(
            "Sending webhook {} to {} for event {} (attempt {}/{})",
            delivery.id,
            delivery.webhook_config.endpoint,
            serde_json::to_string(&delivery.event_type).unwrap_or_default(),
            delivery.attempts + 1,
            delivery.max_retries + 1
        );

        // Log initial webhook attempt to database if this is the first attempt
        if delivery.attempts == 0 {
            self.log_initial_webhook_attempt(&delivery).await;
        }

        let start_time = SystemTime::now();
        let result = self.send_single_request(&delivery).await;
        let end_time = SystemTime::now();
        let duration_ms =
            end_time.duration_since(start_time).map(|d| d.as_millis() as i64).unwrap_or(0);

        match result {
            Ok(response) => {
                let status_code = response.status().as_u16() as i32;
                if response.status().is_success() {
                    let response_text = response.text().await.unwrap_or_default();
                    rrelayer_info!(
                        "Webhook {} delivered successfully to {} (status: {})",
                        delivery.id,
                        delivery.webhook_config.endpoint,
                        status_code
                    );
                    delivery.mark_completed();

                    // Log successful delivery to database
                    self.log_webhook_success(&delivery, status_code, &response_text, duration_ms)
                        .await;
                } else {
                    let error_text = response.text().await.unwrap_or_default();
                    let error =
                        format!("Webhook returned error status: {} - {}", status_code, error_text);
                    warn!(
                        "Webhook {} failed to {} with status {}: {}",
                        delivery.id, delivery.webhook_config.endpoint, status_code, error
                    );
                    self.handle_failed_attempt(
                        &mut delivery,
                        error.clone(),
                        start_time,
                        Some(status_code),
                        Some(error_text),
                        duration_ms,
                    )
                    .await;
                }
            }
            Err(e) => {
                let error = format!("HTTP request failed: {}", e);
                warn!(
                    "Webhook {} request to {} failed: {}",
                    delivery.id, delivery.webhook_config.endpoint, error
                );
                self.handle_failed_attempt(
                    &mut delivery,
                    error,
                    start_time,
                    None,
                    None,
                    duration_ms,
                )
                .await;
            }
        }

        delivery
    }

    /// Send a single HTTP request for the webhook
    async fn send_single_request(
        &self,
        delivery: &WebhookDelivery,
    ) -> Result<Response, reqwest::Error> {
        let signature =
            self.generate_signature(&delivery.payload, &delivery.webhook_config.shared_secret);

        self.client
            .post(&delivery.webhook_config.endpoint)
            .header("Content-Type", "application/json")
            .header("User-Agent", "RRelayer-Webhooks/1.0")
            .header(
                "X-RRelayer-Event",
                serde_json::to_string(&delivery.event_type).unwrap_or_default(),
            )
            .header("X-RRelayer-Signature", signature)
            .header("X-RRelayer-Delivery", delivery.id.to_string())
            .header(
                "X-RRelayer-Timestamp",
                delivery
                    .created_at
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string(),
            )
            .json(&delivery.payload)
            .send()
            .await
    }

    /// Handle a failed webhook attempt
    async fn handle_failed_attempt(
        &self,
        delivery: &mut WebhookDelivery,
        error: String,
        now: SystemTime,
        http_status_code: Option<i32>,
        response_body: Option<String>,
        duration_ms: i64,
    ) {
        let next_retry_delay = if delivery.should_retry() {
            let delay = self.calculate_retry_delay(delivery.attempts);
            Some(delay)
        } else {
            None
        };

        delivery.mark_attempt(now, next_retry_delay);

        if !delivery.should_retry() {
            rrelayer_error!(
                "Webhook {} to {} permanently failed after {} attempts: {}",
                delivery.id,
                delivery.webhook_config.endpoint,
                delivery.attempts,
                error
            );
            delivery.mark_failed(error.clone());

            // Log permanent failure to database
            self.log_webhook_failure(
                delivery,
                &error,
                http_status_code,
                response_body,
                duration_ms,
                true,
            )
            .await;
        } else {
            let retry_delay = next_retry_delay.unwrap_or(0);
            warn!(
                "Webhook {} to {} will retry in {}ms (attempt {} of {}): {}",
                delivery.id,
                delivery.webhook_config.endpoint,
                retry_delay,
                delivery.attempts,
                delivery.max_retries + 1,
                error
            );

            // Log failed attempt to database (not permanent failure yet)
            self.log_webhook_failure(
                delivery,
                &error,
                http_status_code,
                response_body,
                duration_ms,
                false,
            )
            .await;
        }
    }

    /// Calculate exponential backoff delay for retries
    fn calculate_retry_delay(&self, attempt: u32) -> u64 {
        let delay = (self.config.initial_retry_delay_ms as f32)
            * self.config.retry_multiplier.powi(attempt as i32);

        (delay as u64).min(self.config.max_retry_delay_ms)
    }

    /// Generate HMAC signature for webhook verification
    fn generate_signature(&self, payload: &Value, secret: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(&payload_bytes);

        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }

    /// Process multiple webhook deliveries concurrently
    pub async fn send_multiple_webhooks(
        &self,
        deliveries: Vec<WebhookDelivery>,
    ) -> Vec<WebhookDelivery> {
        if deliveries.is_empty() {
            return vec![];
        }

        debug!("Processing {} webhook deliveries", deliveries.len());

        let handles: Vec<_> = deliveries
            .into_iter()
            .map(|delivery| {
                let sender = self.clone();
                tokio::spawn(async move { sender.send_webhook(delivery).await })
            })
            .collect();

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(delivery) => results.push(delivery),
                Err(e) => {
                    rrelayer_error!("Webhook delivery task panicked: {}", e);
                }
            }
        }

        debug!("Completed processing {} webhook deliveries", results.len());
        results
    }

    /// Log initial webhook attempt to database
    async fn log_initial_webhook_attempt(&self, delivery: &WebhookDelivery) {
        // Extract transaction ID and relayer ID from payload if available
        let (transaction_id, relayer_id, chain_id) = self.extract_delivery_identifiers(delivery);

        let request = CreateWebhookDeliveryRequest {
            id: delivery.id,
            webhook_endpoint: delivery.webhook_config.endpoint.clone(),
            event_type: delivery.event_type.clone(),
            status: WebhookDeliveryStatus::Pending,
            transaction_id,
            relayer_id,
            chain_id,
            attempts: 1,
            max_retries: delivery.max_retries as i32,
            payload: delivery.payload.clone(),
            headers: Some(serde_json::json!({
                "X-RRelayer-Event": serde_json::to_string(&delivery.event_type).unwrap_or_default(),
                "X-RRelayer-Delivery": delivery.id.to_string(),
                "X-RRelayer-Timestamp": delivery.created_at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs().to_string()
            })),
            first_attempt_at: Utc::now(),
        };

        if let Err(e) = self.db.create_webhook_delivery(&request).await {
            warn!("Failed to log webhook attempt to database: {}", e);
        }
    }

    /// Log successful webhook delivery to database
    async fn log_webhook_success(
        &self,
        delivery: &WebhookDelivery,
        status_code: i32,
        response_body: &str,
        duration_ms: i64,
    ) {
        let update_request = UpdateWebhookDeliveryRequest {
            id: delivery.id,
            status: WebhookDeliveryStatus::Delivered,
            attempts: delivery.attempts as i32,
            http_status_code: Some(status_code),
            response_body: Some(response_body.to_string()),
            error_message: None,
            last_attempt_at: Utc::now(),
            delivered_at: Some(Utc::now()),
            abandoned_at: None,
            total_duration_ms: Some(duration_ms),
        };

        if let Err(e) = self.db.update_webhook_delivery(&update_request).await {
            warn!("Failed to log webhook success to database: {}", e);
        }
    }

    /// Log failed webhook delivery to database
    async fn log_webhook_failure(
        &self,
        delivery: &WebhookDelivery,
        error: &str,
        status_code: Option<i32>,
        response_body: Option<String>,
        duration_ms: i64,
        is_permanent_failure: bool,
    ) {
        let status = if is_permanent_failure {
            WebhookDeliveryStatus::Abandoned
        } else {
            WebhookDeliveryStatus::Failed
        };

        let update_request = UpdateWebhookDeliveryRequest {
            id: delivery.id,
            status,
            attempts: delivery.attempts as i32,
            http_status_code: status_code,
            response_body,
            error_message: Some(error.to_string()),
            last_attempt_at: Utc::now(),
            delivered_at: None,
            abandoned_at: if is_permanent_failure { Some(Utc::now()) } else { None },
            total_duration_ms: Some(duration_ms),
        };

        if let Err(e) = self.db.update_webhook_delivery(&update_request).await {
            warn!("Failed to log webhook failure to database: {}", e);
        }
    }

    /// Extract transaction ID, relayer ID, and chain ID from delivery payload
    fn extract_delivery_identifiers(
        &self,
        delivery: &WebhookDelivery,
    ) -> (
        Option<crate::transaction::types::TransactionId>,
        Option<crate::relayer::types::RelayerId>,
        Option<crate::network::types::ChainId>,
    ) {
        let payload = &delivery.payload;

        let transaction_id = if let Some(transaction) = payload.get("transaction") {
            transaction
                .get("id")
                .and_then(|id| id.as_str())
                .and_then(|id_str| uuid::Uuid::parse_str(id_str).ok())
                .map(crate::transaction::types::TransactionId::from)
        } else {
            None
        };

        let relayer_id = if let Some(transaction) = payload.get("transaction") {
            transaction
                .get("relayerId")
                .and_then(|id| id.as_str())
                .and_then(|id_str| uuid::Uuid::parse_str(id_str).ok())
                .map(crate::relayer::types::RelayerId::from)
        } else if let Some(signing) = payload.get("signing") {
            signing
                .get("relayerId")
                .and_then(|id| id.as_str())
                .and_then(|id_str| uuid::Uuid::parse_str(id_str).ok())
                .map(crate::relayer::types::RelayerId::from)
        } else {
            None
        };

        let chain_id = if let Some(transaction) = payload.get("transaction") {
            transaction
                .get("chainId")
                .and_then(|id| id.as_u64())
                .map(crate::network::types::ChainId::new)
        } else if let Some(signing) = payload.get("signing") {
            signing
                .get("chainId")
                .and_then(|id| id.as_u64())
                .map(crate::network::types::ChainId::new)
        } else {
            None
        };

        (transaction_id, relayer_id, chain_id)
    }
}

impl Clone for WebhookSender {
    fn clone(&self) -> Self {
        Self { client: self.client.clone(), config: self.config.clone(), db: self.db.clone() }
    }
}
