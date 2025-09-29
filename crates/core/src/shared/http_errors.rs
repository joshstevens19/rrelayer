use crate::postgres::PostgresError;
use reqwest::StatusCode;
use tracing::error;

pub type HttpError = (StatusCode, String);

pub fn internal_server_error(message: Option<String>) -> HttpError {
    (StatusCode::INTERNAL_SERVER_ERROR, message.unwrap_or("Internal server error".to_string()))
}

pub fn bad_request(message: String) -> HttpError {
    (StatusCode::BAD_REQUEST, message)
}

pub fn not_found(message: String) -> HttpError {
    (StatusCode::NOT_FOUND, message)
}

pub fn too_many_requests() -> HttpError {
    (StatusCode::TOO_MANY_REQUESTS, "Too many requests".to_string())
}

pub fn unauthorized(message: Option<String>) -> HttpError {
    (StatusCode::UNAUTHORIZED, message.unwrap_or("Unauthorized".to_string()))
}

pub fn forbidden(message: String) -> HttpError {
    (StatusCode::FORBIDDEN, message)
}

impl From<PostgresError> for HttpError {
    fn from(error: PostgresError) -> HttpError {
        error!("Postgres error occurred - {:?}", error);
        internal_server_error(Some(error.to_string()))
    }
}
