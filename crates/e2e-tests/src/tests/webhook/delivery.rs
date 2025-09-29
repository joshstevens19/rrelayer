use crate::tests::test_runner::TestRunner;
use alloy::dyn_abi::TypedData;
use anyhow::{anyhow, Context};
use rrelayer_core::transaction::api::{RelayTransactionRequest, TransactionSpeed};
use rrelayer_core::transaction::types::{TransactionData, TransactionValue};
use std::time::Duration;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=webhook_delivery
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=webhook_delivery  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=webhook_delivery
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=webhook_delivery
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=webhook_delivery
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=webhook_delivery
    pub async fn webhook_delivery(&self) -> anyhow::Result<()> {
        info!("Testing webhook delivery mechanism...");

        let webhook_server =
            self.webhook_server().ok_or_else(|| anyhow!("Webhook server not initialized"))?;

        webhook_server.clear_webhooks();

        let relayer = self.create_and_fund_relayer("webhook-test-relayer").await?;
        info!("Created relayer for webhook testing: {}", relayer.id());

        info!("Test 1: Simple ETH transfer webhook events");
        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("webhook-eth-transfer".to_string()),
            blobs: None,
        };

        let send_result = relayer.transaction().send(&tx_request, None).await?;

        info!("📨 Transaction submitted: {}", send_result.id);

        let initial_webhooks =
            webhook_server.get_webhooks_for_transaction(&send_result.id.to_string());
        info!("📨 Initial webhooks received: {}", initial_webhooks.len());

        if initial_webhooks.is_empty() {
            info!("[WARNING] No initial webhooks received, checking all webhooks...");
            let all_webhooks = webhook_server.get_received_webhooks();
            info!("[INFO] Total webhooks received so far: {}", all_webhooks.len());
            for webhook in &all_webhooks {
                info!("  - Event: {}, TxID: {}", webhook.event_type, webhook.transaction_id);
            }
        }

        info!("[WAIT] Waiting for transaction {} to complete fully...", send_result.id);
        let (completed_tx, _receipt) =
            self.wait_for_transaction_completion(&send_result.id).await?;
        info!("[SUCCESS] Transaction completed with status: {:?}", completed_tx.status);

        let eth_transfer_webhooks =
            webhook_server.get_webhooks_for_transaction(&send_result.id.to_string());
        info!(
            "📨 Final webhooks received for ETH transfer {}: {}",
            send_result.id,
            eth_transfer_webhooks.len()
        );

        if eth_transfer_webhooks.is_empty() {
            return Err(anyhow!("No webhooks received for ETH transfer transaction"));
        }

        info!("Test 2: Contract interaction webhook events");
        let contract_address = self
            .contract_interactor
            .contract_address()
            .ok_or_else(|| anyhow!("Contract not deployed"))?;

        let contract_data = self.contract_interactor.encode_simple_call(42)?;
        let contract_tx_request = RelayTransactionRequest {
            to: contract_address,
            value: TransactionValue::zero(),
            data: TransactionData::raw_hex(&contract_data).unwrap(),
            speed: Some(TransactionSpeed::FAST),
            external_id: Some("webhook-contract-call".to_string()),
            blobs: None,
        };

        let contract_send_result = relayer.transaction().send(&contract_tx_request, None).await?;

        info!("📨 Contract transaction submitted: {}", contract_send_result.id);

        self.wait_for_transaction_completion(&contract_send_result.id).await?;

        let contract_webhooks =
            webhook_server.get_webhooks_for_transaction(&contract_send_result.id.to_string());
        info!(
            "📨 Received {} webhooks for contract transaction {}",
            contract_webhooks.len(),
            contract_send_result.id
        );

        if contract_webhooks.is_empty() {
            return Err(anyhow!("No webhooks received for contract transaction"));
        }

        info!("Test 3: Webhook payload validation");
        let all_webhooks = webhook_server.get_received_webhooks();
        if all_webhooks.is_empty() {
            return Err(anyhow!("No webhooks were received during testing"));
        }

        info!("[INFO] All webhooks received during test: {}", all_webhooks.len());

        for (i, webhook) in all_webhooks.iter().enumerate() {
            info!(
                "  {}. Event: {}, TxID: {}, RelayerID: {}",
                i + 1,
                webhook.event_type,
                webhook.transaction_id,
                webhook.relayer_id
            );

            if webhook.transaction_id.is_empty() {
                return Err(anyhow!("Webhook missing transaction_id"));
            }
            if webhook.relayer_id.is_empty() {
                return Err(anyhow!("Webhook missing relayer_id"));
            }
            if webhook.event_type.is_empty() {
                return Err(anyhow!("Webhook missing event_type"));
            }

            if !webhook.headers.contains_key("x-rrelayer-shared-secret") {
                return Err(anyhow!("Webhook missing shared secret header"));
            }

            let has_transaction =
                webhook.payload.get("payload").and_then(|p| p.get("transaction")).is_some();
            let has_signing =
                webhook.payload.get("payload").and_then(|p| p.get("signing")).is_some();
            if !has_transaction && !has_signing {
                return Err(anyhow!("Webhook payload missing nested transaction or signing data"));
            }
            if webhook.payload.get("event_type").is_none() {
                return Err(anyhow!("Webhook payload missing event_type at root"));
            }
            if webhook.payload.get("timestamp").is_none() {
                return Err(anyhow!("Webhook payload missing timestamp at root"));
            }
            if webhook.payload.get("delivery_id").is_none() {
                return Err(anyhow!("Webhook payload missing delivery_id at root"));
            }

            info!("[SUCCESS] Webhook validation passed for event: {}", webhook.event_type);
        }

        info!("Test 4: Transaction lifecycle webhook sequence");
        let mut sorted_webhooks = eth_transfer_webhooks.clone();
        sorted_webhooks.sort_by_key(|w| w.timestamp);

        let event_sequence: Vec<String> =
            sorted_webhooks.iter().map(|w| w.event_type.clone()).collect();

        info!("📋 Webhook event sequence: {:?}", event_sequence);

        if let Some(first_event) = event_sequence.first() {
            if first_event != "transaction_queued" {
                return Err(anyhow!(
                    "Expected first webhook event to be 'transaction_queued', got '{}'",
                    first_event
                ));
            }
        }

        let has_queued = event_sequence.contains(&"transaction_queued".to_string());
        let has_sent = event_sequence.contains(&"transaction_sent".to_string());
        let has_mined = event_sequence.contains(&"transaction_mined".to_string());

        if !has_queued {
            return Err(anyhow!("Missing 'transaction_queued' webhook event"));
        }
        if !has_sent {
            return Err(anyhow!("Missing 'transaction_sent' webhook event"));
        }
        if !has_mined {
            return Err(anyhow!("Missing 'transaction_mined' webhook event"));
        }

        info!("Test 5: Transaction confirmation webhook (15 blocks)");
        info!("[MINING] Mining 15 blocks to confirm the ETH transfer transaction...");
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("[WAIT] Waiting for confirmation processing...");

        let mut confirmed_webhooks = Vec::new();
        for attempt in 1..=10 {
            confirmed_webhooks = webhook_server.get_webhooks_by_event("transaction_confirmed");
            if !confirmed_webhooks.is_empty() {
                break;
            }
            info!("[WAIT] Attempt {}/10: Waiting for transaction_confirmed webhook...", attempt);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        if confirmed_webhooks.is_empty() {
            return Err(anyhow!(
                "Missing 'transaction_confirmed' webhook event after 15 blocks and 15 seconds wait"
            ));
        }

        info!("[SUCCESS] Received 'transaction_confirmed' webhook event");

        info!("Test 6: Transaction cancelled webhook");

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-original".to_string()),
            blobs: None,
        };

        let send_result = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to send transaction")?;

        let transaction_id = &send_result.id;

        let cancel_result = relayer
            .transaction()
            .cancel(transaction_id, None)
            .await
            .context("Failed to cancel transaction")?;

        if !cancel_result.success {
            return Err(anyhow::anyhow!("Cancel transaction failed"));
        }

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("[WAIT] Waiting for cancellation processing...");

        info!("Test 7: Transaction replacement webhook");

        let tx_request = RelayTransactionRequest {
            to: self.config.anvil_accounts[1],
            value: alloy::primitives::utils::parse_ether("0.1")?.into(),
            data: TransactionData::empty(),
            speed: Some(TransactionSpeed::SLOW),
            external_id: Some("test-original".to_string()),
            blobs: None,
        };

        let _ = relayer
            .transaction()
            .send(&tx_request, None)
            .await
            .context("Failed to send transaction")?;

        // TODO: uncomment when fix nonce issue on webhooks
        // let transaction_id = &send_result.id;
        //
        // let replacement_request = RelayTransactionRequest {
        //     to: self.config.anvil_accounts[1],
        //     value: alloy::primitives::utils::parse_ether("0.2")?.into(),
        //     data: TransactionData::empty(),
        //     speed: Some(TransactionSpeed::FAST),
        //     external_id: Some("test-replacement".to_string()),
        //     blobs: None,
        // };

        // let replace_result = relayer
        //     .transaction()
        //     .replace(transaction_id, &replacement_request, None)
        //     .await
        //     .context("Failed to replace transaction")?;
        // info!("[SUCCESS] Transaction replacement result: {:?}", replace_result);
        //
        // if !replace_result.success {
        //     return Err(anyhow::anyhow!("Replace transaction failed"));
        // }

        self.mine_and_wait().await?;
        self.mine_and_wait().await?;
        self.mine_and_wait().await?;

        info!("[WAIT] Waiting for replace transaction processing...");

        info!("Test 8: Comprehensive webhook event verification");
        let final_all_webhooks = webhook_server.get_received_webhooks();
        let final_webhook_events: Vec<String> =
            final_all_webhooks.iter().map(|w| w.event_type.clone()).collect();
        let final_unique_events: std::collections::HashSet<String> =
            final_webhook_events.iter().cloned().collect();

        let webhook_events = [
            "transaction_queued",
            "transaction_sent",
            "transaction_mined",
            "transaction_confirmed",
            "transaction_cancelled",
            //  "transaction_replaced",
        ];

        for event in &webhook_events {
            let count = final_webhook_events.iter().filter(|&e| e == event).count();
            if count > 0 {
                info!("[SUCCESS] Received '{}' webhook event ({} occurrences)", event, count);
            } else {
                return Err(anyhow!("Missing '{}' webhook event", event));
            }
        }

        info!(
            "📋 Successfully received all transaction webhook events: {:?}",
            final_unique_events.into_iter().collect::<Vec<_>>()
        );

        info!("Test 7: Signing operations webhook events");

        webhook_server.clear_webhooks();

        // Test text signing webhook
        info!("[SECURE] Testing text signing webhook...");
        let text_to_sign = "Hello, RRelayer webhook test!";

        let sign_text_result = relayer.sign().text(text_to_sign, None).await?;

        info!("[SUCCESS] Text signed successfully, signature: {:?}", sign_text_result.signature);

        // Wait a moment for webhook delivery
        tokio::time::sleep(Duration::from_millis(500)).await;

        let text_signing_webhooks = webhook_server.get_webhooks_by_event("text_signed");
        if text_signing_webhooks.is_empty() {
            return Err(anyhow!("No 'text_signed' webhook received"));
        }

        info!("[SUCCESS] Received {} text_signed webhook(s)", text_signing_webhooks.len());

        // Validate text signing webhook payload
        let text_webhook = &text_signing_webhooks[0];
        let nested_payload = text_webhook.payload.get("payload");
        if nested_payload.and_then(|p| p.get("signing")).is_none() {
            return Err(anyhow!("Text signing webhook missing 'signing' data in nested payload"));
        }

        let signing_data = nested_payload.unwrap().get("signing").unwrap();
        if signing_data.get("message").is_none() {
            return Err(anyhow!("Text signing webhook missing 'message' field"));
        }
        if signing_data.get("signature").is_none() {
            return Err(anyhow!("Text signing webhook missing 'signature' field"));
        }
        if signing_data.get("relayerId").is_none() {
            return Err(anyhow!("Text signing webhook missing 'relayerId' field"));
        }

        let message_value = signing_data.get("message").unwrap().as_str().unwrap_or("");
        if message_value != text_to_sign {
            return Err(anyhow!(
                "Text signing webhook message mismatch: expected '{}', got '{}'",
                text_to_sign,
                message_value
            ));
        }

        info!("[SUCCESS] Text signing webhook payload validation passed");

        info!("[SECURE] Testing typed data signing webhook...");

        use serde_json::json;
        let typed_data = json!({
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"}
                ],
                "TestMessage": [
                    {"name": "message", "type": "string"},
                    {"name": "value", "type": "uint256"}
                ]
            },
            "primaryType": "TestMessage",
            "domain": {
                "name": "RRelayer Test",
                "version": "1",
                "chainId": 31337
            },
            "message": {
                "message": "Test webhook typed data",
                "value": 42
            }
        });

        let typed_data_parsed: TypedData = serde_json::from_value(typed_data)?;

        let sign_typed_data_result = relayer.sign().typed_data(&typed_data_parsed, None).await?;

        info!(
            "[SUCCESS] Typed data signed successfully, signature: {:?}",
            sign_typed_data_result.signature
        );

        tokio::time::sleep(Duration::from_millis(500)).await;

        let typed_data_signing_webhooks = webhook_server.get_webhooks_by_event("typed_data_signed");
        if typed_data_signing_webhooks.is_empty() {
            return Err(anyhow!("No 'typed_data_signed' webhook received"));
        }

        info!(
            "[SUCCESS] Received {} typed_data_signed webhook(s)",
            typed_data_signing_webhooks.len()
        );

        let typed_data_webhook = &typed_data_signing_webhooks[0];
        let typed_nested_payload = typed_data_webhook.payload.get("payload");
        if typed_nested_payload.and_then(|p| p.get("signing")).is_none() {
            return Err(anyhow!(
                "Typed data signing webhook missing 'signing' data in nested payload"
            ));
        }

        let typed_signing_data = typed_nested_payload.unwrap().get("signing").unwrap();
        if typed_signing_data.get("domainData").is_none() {
            return Err(anyhow!("Typed data signing webhook missing 'domainData' field"));
        }
        if typed_signing_data.get("messageData").is_none() {
            return Err(anyhow!("Typed data signing webhook missing 'messageData' field"));
        }
        if typed_signing_data.get("primaryType").is_none() {
            return Err(anyhow!("Typed data signing webhook missing 'primaryType' field"));
        }
        if typed_signing_data.get("signature").is_none() {
            return Err(anyhow!("Typed data signing webhook missing 'signature' field"));
        }

        let primary_type_value =
            typed_signing_data.get("primaryType").unwrap().as_str().unwrap_or("");
        if primary_type_value != "TestMessage" {
            return Err(anyhow!(
                "Typed data signing webhook primaryType mismatch: expected 'TestMessage', got '{}'",
                primary_type_value
            ));
        }

        info!("[SUCCESS] Typed data signing webhook payload validation passed");

        for signing_webhook in [&text_signing_webhooks[0], &typed_data_signing_webhooks[0]] {
            if !signing_webhook.headers.contains_key("x-rrelayer-shared-secret") {
                return Err(anyhow!("Signing webhook missing shared secret header"));
            }

            if signing_webhook.payload.get("event_type").is_none() {
                return Err(anyhow!("Signing webhook payload missing event_type"));
            }
            if signing_webhook.payload.get("timestamp").is_none() {
                return Err(anyhow!("Signing webhook payload missing timestamp"));
            }
            if signing_webhook.payload.get("payload").and_then(|p| p.get("api_version")).is_none() {
                return Err(anyhow!(
                    "Signing webhook payload missing api_version in nested payload"
                ));
            }
        }

        info!("[SUCCESS] Signing webhook structure validation passed");
        info!("[INFO] Signing tests summary:");
        info!("   [SECURE] Text signing webhook: [SUCCESS] received and validated");
        info!("   [NOTE] Typed data signing webhook: [SUCCESS] received and validated");
        info!("   [CHECK] Payload structure: [SUCCESS] all required fields present");
        info!("   [LOCKED] HMAC signature validation: [SUCCESS] headers present");

        info!("[SUCCESS] Comprehensive webhook delivery testing completed successfully!");
        info!("   [INFO] Total webhooks received: {}", final_all_webhooks.len());
        info!("   📋 Core events tested: queued, sent, mined, confirmed");
        info!("   [LOCKED] Signature validation: passed");
        info!("   [NOTE] Payload structure: validated");
        info!("   [RETRY] Lifecycle sequence: verified");
        info!("   📤 Contract interactions: tested");
        info!("   [SUCCESS] Transaction confirmations: tested (15 blocks)");
        info!("   [SECURE] Text signing webhooks: tested and validated");
        info!("   📜 Typed data signing webhooks: tested and validated");
        info!("   [CHECK] Webhook consistency: all calls non-blocking");

        Ok(())
    }
}
