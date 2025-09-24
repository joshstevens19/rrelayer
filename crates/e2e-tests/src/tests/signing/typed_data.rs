use crate::tests::test_runner::TestRunner;
use alloy::dyn_abi::TypedData;
use anyhow::Context;
use rrelayer_core::common_types::PagingContext;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=signing_typed_data
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=signing_typed_data
    /// RELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=signing_typed_data
    /// RELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=signing_typed_data
    /// RELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=signing_typed_data
    /// RELAYER_PROVIDERS="turnkey" make run-test-debug TEST=signing_typed_data
    pub async fn signing_typed_data(&self) -> anyhow::Result<()> {
        info!("Testing typed data signing...");

        let relayer = self.create_and_fund_relayer("signing-typed-data-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let typed_data_json = serde_json::json!({
            "types": {
                "EIP712Domain": [
                    {"name": "name", "type": "string"},
                    {"name": "version", "type": "string"},
                    {"name": "chainId", "type": "uint256"},
                    {"name": "verifyingContract", "type": "address"}
                ],
                "Mail": [
                    {"name": "from", "type": "Person"},
                    {"name": "to", "type": "Person"},
                    {"name": "contents", "type": "string"}
                ],
                "Person": [
                    {"name": "name", "type": "string"},
                    {"name": "wallet", "type": "address"}
                ]
            },
            "primaryType": "Mail",
            "domain": {
                "name": "RRelayer Test",
                "version": "1",
                "chainId": self.config.chain_id,
                "verifyingContract": "0x0000000000000000000000000000000000000000"
            },
            "message": {
                "from": {
                    "name": "Alice",
                    "wallet": "0x1234567890123456789012345678901234567890"
                },
                "to": {
                    "name": "Bob",
                    "wallet": "0x0987654321098765432109876543210987654321"
                },
                "contents": "Hello from E2E test!"
            }
        });

        let typed_data: TypedData =
            serde_json::from_value(typed_data_json).context("Failed to create typed data")?;

        let sign_result = self
            .relayer_client
            .sdk
            .sign
            .sign_typed_data(&relayer.id, &typed_data, None)
            .await
            .context("Failed to sign typed data")?;

        info!("Signed typed data. Signature: {}", sign_result.signature);

        info!("[SUCCESS] Got typed data signature: {:?}", sign_result.signature);

        let paging = PagingContext { limit: 10, offset: 0 };
        let history = self
            .relayer_client
            .sdk
            .sign
            .get_typed_data_history(&relayer.id, &paging)
            .await
            .context("Failed to get typed data signing history")?;

        info!("Typed data signing history has {} entries", history.items.len());

        let signed_entry = history.items.iter().find(|entry| {
            if let Some(domain) = entry.domain_data.get("name") {
                domain.as_str() == Some("RRelayer Test")
            } else {
                false
            }
        });

        if let Some(entry) = signed_entry {
            info!("[SUCCESS] Found signed typed data in history: {:?}", entry.domain_data);
            info!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed typed data not found in history"));
        }

        info!("[SUCCESS] Typed data signing works correctly");
        Ok(())
    }
}
