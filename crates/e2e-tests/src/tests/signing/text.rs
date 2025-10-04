use crate::tests::test_runner::TestRunner;
use anyhow::Context;
use rrelayer_core::common_types::PagingContext;
use tracing::info;

impl TestRunner {
    /// run single with:
    /// RRELAYER_PROVIDERS="raw" make run-test-debug TEST=signing_text
    /// RRELAYER_PROVIDERS="privy" make run-test-debug TEST=signing_text  
    /// RRELAYER_PROVIDERS="aws_secret_manager" make run-test-debug TEST=signing_text
    /// RRELAYER_PROVIDERS="aws_kms" make run-test-debug TEST=signing_text
    /// RRELAYER_PROVIDERS="gcp_secret_manager" make run-test-debug TEST=signing_text
    /// RRELAYER_PROVIDERS="turnkey" make run-test-debug TEST=signing_text
    /// RRELAYER_PROVIDERS="pkcs11" make run-test-debug TEST=signing_text
    pub async fn signing_text(&self) -> anyhow::Result<()> {
        info!("Testing text signing...");

        let relayer = self.create_and_fund_relayer("signing-text-relayer").await?;
        info!("Created relayer: {:?}", relayer);

        let test_message = "Hello, RRelayer E2E Test!";

        let sign_result =
            relayer.sign().text(test_message, None).await.context("Failed to sign text message")?;

        info!("Signed message. Signature: {}", sign_result.signature);

        info!("[SUCCESS] Got signature: {:?}", sign_result.signature);

        let paging = PagingContext { limit: 10, offset: 0 };
        let history = relayer
            .sign()
            .text_history(&paging)
            .await
            .context("Failed to get text signing history")?;

        info!("Text signing history has {} entries", history.items.len());

        let signed_message = history.items.iter().find(|entry| entry.message == test_message);

        if let Some(entry) = signed_message {
            info!("[SUCCESS] Found signed message in history: {}", entry.message);
            info!("   Signature: {}", entry.signature);
        } else {
            return Err(anyhow::anyhow!("Signed message not found in history"));
        }

        info!("[SUCCESS] Text signing works correctly");
        Ok(())
    }
}
