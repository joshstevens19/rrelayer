use axum::http::HeaderMap;

pub type ApiKey = String;

/// Extracts an API key from HTTP headers.
///
/// Looks for the 'x-api-key' header and converts it to a string.
///
/// # Arguments
/// * `headers` - The HTTP header map to search
///
/// # Returns
/// * `Some(ApiKey)` - The API key string if the header exists and is valid UTF-8
/// * `None` - If the header is missing or contains invalid UTF-8
pub fn api_key_from_headers(headers: &HeaderMap) -> Option<ApiKey> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|api_key| api_key.to_string())
}
