use crate::postgres::PostgresError;
use reqwest::StatusCode;
use tracing::error;

pub type HttpError = (StatusCode, String);

pub fn bad_request(message: String) -> HttpError {
    (StatusCode::BAD_REQUEST, message)
}

impl From<PostgresError> for HttpError {
    fn from(error: PostgresError) -> HttpError {
        error!("Postgres error occurred - {:?}", error);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
    }
}
