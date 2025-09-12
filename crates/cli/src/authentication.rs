use crate::error::CliError;
use rrelayer_sdk::SDK;

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

pub async fn check_authenticate(sdk: &SDK) -> Result<(), CliError> {
    check_api_running(sdk).await?;

    match sdk.test_auth().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("❌ Basic authentication failed: {}", e);
            eprintln!(
                "Please check your RRELAYER_AUTH_USERNAME and RRELAYER_AUTH_PASSWORD environment variables."
            );
            Err(CliError::Authentication(format!("Basic authentication failed: {}", e)))
        }
    }
}
