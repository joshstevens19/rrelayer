use rrelayer_sdk::SDK;
use crate::{commands::keystore::ProjectLocation, error::CliError};

/// Verifies that the rrelayer API server is running and accessible with basic auth.
///
/// Performs a health check request and tests basic authentication to ensure 
/// the API server is available and configured correctly.
///
/// # Arguments
/// * `sdk` - SDK instance configured with API endpoint and basic auth credentials
///
/// # Returns
/// * `Ok(())` - API server is running and basic auth is working
/// * `Err(CliError)` - API server is unreachable, not running, or auth failed
pub async fn check_api_running(sdk: &SDK) -> Result<(), CliError> {
    match sdk.health.check().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: API server is not running or is unreachable.");
            eprintln!("Please start the API server before continuing.");
            eprintln!("Details: {}", e);

            Err(CliError::Api(
                "The API server is not running. Please start it before continuing.".to_string(),
            ))
        }
    }
}

/// Handles authentication for CLI commands using basic auth.
///
/// With the simplified basic auth system, this function now only needs to verify
/// that the API server is running and that basic auth credentials are working.
/// The SDK is already configured with basic auth credentials from environment variables.
///
/// # Arguments
/// * `sdk` - SDK instance with basic auth already configured
/// * `account` - Account identifier (kept for compatibility, not used with basic auth)
/// * `project_location` - Project configuration (kept for compatibility)
///
/// # Returns
/// * `Ok(())` - Authentication successful
/// * `Err(CliError)` - API unavailable or basic auth failed
pub async fn handle_authenticate(
    sdk: &mut SDK,
    _account: &str, // Not used with basic auth but kept for compatibility
    _project_location: &ProjectLocation, // Not used with basic auth but kept for compatibility
) -> Result<(), CliError> {
    // Check if API server is running
    check_api_running(sdk).await?;
    
    // Test basic auth by calling the auth status endpoint
    match sdk.test_auth().await {
        Ok(_) => {
            println!("✅ Basic authentication successful!");
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Basic authentication failed: {}", e);
            eprintln!("Please check your RRELAYER_AUTH_USERNAME and RRELAYER_AUTH_PASSWORD environment variables.");
            Err(CliError::Authentication(format!("Basic authentication failed: {}", e)))
        }
    }
}