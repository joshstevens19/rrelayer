use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::sync::oneshot;
use tracing::info;
use warp::{
    http::StatusCode,
    reply::{self, WithStatus},
    Filter, Rejection, Reply,
};

/// Webhook event received from RRelayer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedWebhook {
    pub event_type: String,
    pub transaction_id: String,
    pub relayer_id: String,
    pub timestamp: SystemTime,
    pub payload: serde_json::Value,
    pub headers: HashMap<String, String>,
}

/// Webhook server for E2E testing
#[derive(Clone)]
pub struct WebhookTestServer {
    /// Received webhooks storage
    received_webhooks: Arc<Mutex<Vec<ReceivedWebhook>>>,
    /// Expected shared secret for verification
    shared_secret: String,
    /// Server shutdown signal
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl WebhookTestServer {
    /// Create a new webhook test server
    pub fn new(shared_secret: String) -> Self {
        Self {
            received_webhooks: Arc::new(Mutex::new(Vec::new())),
            shared_secret,
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the webhook server on the specified port
    pub async fn start(&self, port: u16) -> Result<()> {
        let server = self.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Store shutdown signal
        {
            let mut tx = self.shutdown_tx.lock().unwrap();
            *tx = Some(shutdown_tx);
        }

        info!("Starting webhook test server on port {}", port);

        // Create webhook endpoint
        let webhook_route = warp::path("webhook")
            .and(warp::post())
            .and(warp::header::headers_cloned())
            .and(warp::body::json())
            .and(warp::any().map(move || server.clone()))
            .and_then(handle_webhook);

        // Health check endpoint
        let health_route = warp::path("health").and(warp::get()).map(|| "OK");

        let routes = webhook_route.or(health_route);

        let (_, server_future) =
            warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], port), async {
                info!("[WAIT] Webhook server waiting for shutdown signal...");
                shutdown_rx.await.ok();
                info!("📡 Webhook server received shutdown signal");
            });

        info!("Webhook test server started on http://localhost:{}", port);
        server_future.await;
        info!("Webhook test server stopped");

        Ok(())
    }

    /// Stop the webhook server
    pub fn stop(&self) {
        info!("[STOP] stop() called on webhook server");
        let mut tx = self.shutdown_tx.lock().unwrap();
        if let Some(sender) = tx.take() {
            info!("[STOP] Sending shutdown signal to webhook server");
            let _ = sender.send(());
        } else {
            info!("[WARNING] No shutdown sender available - server may already be stopped");
        }
    }

    /// Get all received webhooks
    pub fn get_received_webhooks(&self) -> Vec<ReceivedWebhook> {
        self.received_webhooks.lock().unwrap().clone()
    }

    /// Get webhooks for a specific transaction
    pub fn get_webhooks_for_transaction(&self, transaction_id: &str) -> Vec<ReceivedWebhook> {
        self.received_webhooks
            .lock()
            .unwrap()
            .iter()
            .filter(|webhook| webhook.transaction_id == transaction_id)
            .cloned()
            .collect()
    }

    /// Get webhooks by event type
    pub fn get_webhooks_by_event(&self, event_type: &str) -> Vec<ReceivedWebhook> {
        self.received_webhooks
            .lock()
            .unwrap()
            .iter()
            .filter(|webhook| webhook.event_type == event_type)
            .cloned()
            .collect()
    }

    /// Clear all received webhooks
    pub fn clear_webhooks(&self) {
        self.received_webhooks.lock().unwrap().clear();
    }

    /// Count webhooks by event type
    pub fn count_webhooks_by_event(&self, event_type: &str) -> usize {
        self.received_webhooks
            .lock()
            .unwrap()
            .iter()
            .filter(|webhook| webhook.event_type == event_type)
            .count()
    }

    /// Wait for a webhook with timeout
    pub async fn wait_for_webhook(
        &self,
        transaction_id: &str,
        event_type: &str,
        timeout_seconds: u64,
    ) -> Option<ReceivedWebhook> {
        let timeout = tokio::time::Duration::from_secs(timeout_seconds);
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            let webhooks = self.get_webhooks_for_transaction(transaction_id);
            if let Some(webhook) = webhooks.iter().find(|w| w.event_type == event_type) {
                return Some(webhook.clone());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        None
    }

    /// Verify HMAC signature
    fn verify_signature(&self, payload: &serde_json::Value, signature: &str) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
        let mut mac = HmacSha256::new_from_slice(self.shared_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(&payload_bytes);

        let result = mac.finalize();
        let expected = format!("sha256={}", hex::encode(result.into_bytes()));

        signature == expected
    }

    /// Record a received webhook
    fn record_webhook(&self, webhook: ReceivedWebhook) {
        info!(
            "Received webhook: {} for transaction {} with event {}",
            webhook.transaction_id, webhook.transaction_id, webhook.event_type
        );
        self.received_webhooks.lock().unwrap().push(webhook);
    }
}

/// Handle incoming webhook requests
async fn handle_webhook(
    headers: warp::hyper::HeaderMap,
    payload: serde_json::Value,
    server: WebhookTestServer,
) -> Result<impl Reply, Rejection> {
    // Extract headers
    let mut header_map = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            header_map.insert(key.to_string(), value_str.to_string());
        }
    }

    // Get required headers
    let event_type = header_map
        .get("x-rrelayer-event")
        .unwrap_or(&"unknown".to_string())
        .trim_matches('"')
        .to_string();

    let signature = header_map.get("x-rrelayer-signature").map(|s| s.as_str()).unwrap_or("");

    let delivery_id =
        header_map.get("x-rrelayer-delivery").unwrap_or(&"unknown".to_string()).clone();

    // Verify signature
    if !server.verify_signature(&payload, signature) {
        info!("Webhook signature verification failed for delivery {}", delivery_id);
        return Ok(reply::with_status("Invalid signature", StatusCode::UNAUTHORIZED));
    }

    // Extract transaction data
    let transaction_id = payload["transaction"]["id"].as_str().unwrap_or("unknown").to_string();

    let relayer_id = payload["transaction"]["relayerId"].as_str().unwrap_or("unknown").to_string();

    // Record the webhook
    let webhook = ReceivedWebhook {
        event_type,
        transaction_id,
        relayer_id,
        timestamp: SystemTime::now(),
        payload,
        headers: header_map,
    };

    server.record_webhook(webhook);

    Ok(reply::with_status("OK", StatusCode::OK))
}
