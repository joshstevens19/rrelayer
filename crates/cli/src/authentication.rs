use crate::error::CliError;
use crate::print_error_message;
use rrelayer_sdk::SDK;

pub async fn check_api_running(sdk: &SDK) -> Result<(), CliError> {
    match sdk.health.check().await {
        Ok(_) => Ok(()),
        Err(e) => {
            print_error_message("Error: API server is not running or is unreachable.");
            print_error_message("Please start the API server before continuing.");
            print_error_message(&format!("Details: {}", e));

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
            print_error_message(&format!("❌ Basic authentication failed: {}", e));
            print_error_message(
                "Please check your RRELAYER_AUTH_USERNAME and RRELAYER_AUTH_PASSWORD environment variables.",
            );
            Err(CliError::Authentication(format!("Basic authentication failed: {}", e)))
        }
    }
}
