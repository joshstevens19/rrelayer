use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::HeaderMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::yaml::{JwtHs256Config, SignatureScheme};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SignatureVerificationError {
    #[error("Missing signature header `{0}`")]
    MissingSignatureHeader(String),
    #[error("Header `{0}` contains non-ASCII characters")]
    NonAsciiHeader(String),
    #[error("Secret env variable `{0}` is not set")]
    SecretEnvMissing(String),
    #[error("Secret env variable `{0}` is empty")]
    SecretEnvEmpty(String),
    #[error("Invalid or expired JWT in signature header")]
    InvalidJwt,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct JwtClaims {
    exp: u64,
    #[serde(rename = "userEmail", default)]
    #[allow(dead_code)]
    user_email: Option<String>,
}

pub fn verify_request_signature(
    scheme: &SignatureScheme,
    headers: &HeaderMap,
) -> Result<(), SignatureVerificationError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SignatureVerificationError::InvalidJwt)?
        .as_secs();
    verify_request_signature_at(scheme, headers, now)
}

fn verify_request_signature_at(
    scheme: &SignatureScheme,
    headers: &HeaderMap,
    now: u64,
) -> Result<(), SignatureVerificationError> {
    match scheme {
        SignatureScheme::JwtHs256(config) => verify_jwt_hs256_at(config, headers, now),
    }
}

fn verify_jwt_hs256_at(
    config: &JwtHs256Config,
    headers: &HeaderMap,
    now: u64,
) -> Result<(), SignatureVerificationError> {
    // Static configuration preflight guarantees this exists at startup. Check
    // it first so later operator-side loss is never misreported as caller 401.
    let secret = std::env::var(&config.secret_env)
        .map_err(|_| SignatureVerificationError::SecretEnvMissing(config.secret_env.clone()))?;
    if secret.trim().is_empty() {
        return Err(SignatureVerificationError::SecretEnvEmpty(config.secret_env.clone()));
    }

    let header_name = config.signature_header();
    let token = match headers.get(header_name) {
        None => {
            return Err(SignatureVerificationError::MissingSignatureHeader(header_name.to_string()))
        }
        Some(value) => value
            .to_str()
            .map_err(|_| SignatureVerificationError::NonAsciiHeader(header_name.to_string()))?,
    }
    .trim();

    if token.is_empty() {
        return Err(SignatureVerificationError::InvalidJwt);
    }

    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    // Keep `exp` required, but perform its boundary check explicitly so the
    // exact zero-leeway contract is deterministic and unit-testable.
    validation.validate_exp = false;
    let claims =
        decode::<JwtClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
            .map_err(|_| SignatureVerificationError::InvalidJwt)?
            .claims;

    if claims.exp <= now {
        return Err(SignatureVerificationError::InvalidJwt);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::{json, Value};

    const SECRET: &str = "super-secret-do-not-use";

    fn scheme(environment: &str, header: Option<&str>) -> SignatureScheme {
        SignatureScheme::JwtHs256(JwtHs256Config {
            secret_env: environment.to_string(),
            signature_header: header.map(str::to_string),
        })
    }

    fn token(secret: &str, algorithm: Algorithm, claims: Value) -> String {
        encode(&Header::new(algorithm), &claims, &EncodingKey::from_secret(secret.as_bytes()))
            .unwrap()
    }

    fn headers(name: &'static str, token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, HeaderValue::from_str(token).unwrap());
        headers
    }

    #[test]
    fn verifies_raw_hs256_in_default_and_custom_headers_with_trimmed_whitespace() {
        for (environment, configured, actual) in [
            ("RRELAYER_TEST_JWT_DEFAULT", None, "x-appsmith-signature"),
            ("RRELAYER_TEST_JWT_CUSTOM", Some("x-custom-signature"), "x-custom-signature"),
        ] {
            // SAFETY: every case owns a unique environment variable.
            unsafe { std::env::set_var(environment, SECRET) };
            let token = token(SECRET, Algorithm::HS256, json!({ "exp": 101 }));
            verify_request_signature_at(
                &scheme(environment, configured),
                &headers(actual, &format!("  {token}  ")),
                100,
            )
            .unwrap();
        }
    }

    #[test]
    fn requires_exp_and_enforces_zero_leeway_boundary() {
        const ENV: &str = "RRELAYER_TEST_JWT_EXP_BOUNDARY";
        // SAFETY: this test owns a unique environment variable.
        unsafe { std::env::set_var(ENV, SECRET) };

        for claims in [json!({}), json!({ "exp": 99 }), json!({ "exp": 100 })] {
            let token = token(SECRET, Algorithm::HS256, claims);
            assert_eq!(
                verify_request_signature_at(
                    &scheme(ENV, None),
                    &headers("x-appsmith-signature", &token),
                    100,
                ),
                Err(SignatureVerificationError::InvalidJwt)
            );
        }

        let token = token(SECRET, Algorithm::HS256, json!({ "exp": 101 }));
        verify_request_signature_at(
            &scheme(ENV, None),
            &headers("x-appsmith-signature", &token),
            100,
        )
        .unwrap();
    }

    #[test]
    fn rejects_bearer_empty_wrong_secret_algorithm_and_malformed_tokens() {
        const ENV: &str = "RRELAYER_TEST_JWT_INVALID";
        // SAFETY: this test owns a unique environment variable.
        unsafe { std::env::set_var(ENV, SECRET) };
        let valid = token(SECRET, Algorithm::HS256, json!({ "exp": 101 }));

        for value in [
            format!("Bearer {valid}"),
            "   ".to_string(),
            token("wrong-secret", Algorithm::HS256, json!({ "exp": 101 })),
            token(SECRET, Algorithm::HS384, json!({ "exp": 101 })),
            "not.a.jwt".to_string(),
        ] {
            assert_eq!(
                verify_request_signature_at(
                    &scheme(ENV, None),
                    &headers("x-appsmith-signature", &value),
                    100,
                ),
                Err(SignatureVerificationError::InvalidJwt)
            );
        }
    }

    #[test]
    fn distinguishes_missing_non_ascii_headers_and_operator_secret_loss() {
        const PRESENT: &str = "RRELAYER_TEST_JWT_HEADER_ERRORS";
        const MISSING: &str = "RRELAYER_TEST_JWT_MISSING";
        const EMPTY: &str = "RRELAYER_TEST_JWT_EMPTY";
        // SAFETY: this test owns both unique environment variables.
        unsafe {
            std::env::set_var(PRESENT, SECRET);
            std::env::remove_var(MISSING);
            std::env::set_var(EMPTY, "   ");
        }

        assert!(matches!(
            verify_request_signature_at(&scheme(PRESENT, None), &HeaderMap::new(), 100),
            Err(SignatureVerificationError::MissingSignatureHeader(_))
        ));
        let mut non_ascii = HeaderMap::new();
        non_ascii.insert("x-appsmith-signature", HeaderValue::from_bytes(&[0xff]).unwrap());
        assert!(matches!(
            verify_request_signature_at(&scheme(PRESENT, None), &non_ascii, 100),
            Err(SignatureVerificationError::NonAsciiHeader(_))
        ));

        let token_headers = headers("x-appsmith-signature", "not.a.jwt");
        assert_eq!(
            verify_request_signature_at(&scheme(MISSING, None), &token_headers, 100),
            Err(SignatureVerificationError::SecretEnvMissing(MISSING.to_string()))
        );
        assert_eq!(
            verify_request_signature_at(&scheme(EMPTY, None), &token_headers, 100),
            Err(SignatureVerificationError::SecretEnvEmpty(EMPTY.to_string()))
        );
    }
}
