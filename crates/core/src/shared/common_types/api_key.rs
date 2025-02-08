use axum::http::HeaderMap;

pub type ApiKey = String;

pub fn api_key_from_headers(headers: &HeaderMap) -> Option<ApiKey> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|api_key| api_key.to_string())
}
