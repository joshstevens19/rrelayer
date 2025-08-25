use crate::{rrelayer_error, rrelayer_info};
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, warn};

use super::types::{WebhookDelivery, WebhookDeliveryConfig};

pub struct WebhookSender {
    client: Client,
    config: WebhookDeliveryConfig,
}

impl WebhookSender {
    pub fn new(config: WebhookDeliveryConfig) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds as u64))
            .user_agent("RRelayer-Webhooks/1.0")
            .build()?;

        Ok(Self { client, config })
    }

    /// Send a webhook with retry logic
    pub async fn send_webhook(&self, mut delivery: WebhookDelivery) -> WebhookDelivery {
        rrelayer_info!(
            "Sending webhook {} to {} for event {} (attempt {}/{})",
            delivery.id,
            delivery.webhook_config.endpoint,
            serde_json::to_string(&delivery.event_type).unwrap_or_default(),
            delivery.attempts + 1,
            delivery.max_retries + 1
        );

        let now = SystemTime::now();
        let result = self.send_single_request(&delivery).await;

        match result {
            Ok(response) => {
                if response.status().is_success() {
                    rrelayer_info!(
                        "Webhook {} delivered successfully to {} (status: {})",
                        delivery.id,
                        delivery.webhook_config.endpoint,
                        response.status()
                    );
                    delivery.mark_completed();
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    let error =
                        format!("Webhook returned error status: {} - {}", status, error_text);
                    warn!(
                        "Webhook {} failed to {} with status {}: {}",
                        delivery.id, delivery.webhook_config.endpoint, status, error
                    );
                    self.handle_failed_attempt(&mut delivery, error, now).await;
                }
            }
            Err(e) => {
                let error = format!("HTTP request failed: {}", e);
                warn!(
                    "Webhook {} request to {} failed: {}",
                    delivery.id, delivery.webhook_config.endpoint, error
                );
                self.handle_failed_attempt(&mut delivery, error, now).await;
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
            delivery.mark_failed(error);
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
}

impl Clone for WebhookSender {
    fn clone(&self) -> Self {
        Self { client: self.client.clone(), config: self.config.clone() }
    }
}
