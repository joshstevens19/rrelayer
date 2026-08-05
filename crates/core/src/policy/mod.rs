pub mod ip_allowlist;
pub mod signature;

pub use ip_allowlist::{ip_allowed, validate_ip_allowlist, IpAllowlistError};
pub use signature::{verify_request_signature, SignatureVerificationError};

use crate::common_types::EvmAddress;
use crate::middleware::policy::PolicyContext;
use crate::shared::{forbidden, internal_server_error, unauthorized, HttpError};
use crate::yaml::NetworkPermissionsConfig;
use axum::http::HeaderMap;

pub(crate) fn validate_request_policy_entries(
    permissions: &[NetworkPermissionsConfig],
    context: &PolicyContext,
    headers: &HeaderMap,
    relayer: &EvmAddress,
) -> Result<(), HttpError> {
    for permission in permissions {
        if !permission.relayers.contains(relayer) {
            continue;
        }

        // Control order is part of the external status contract: a request
        // failing both controls is forbidden by IP before JWT is evaluated.
        if let Some(rules) = permission.ip_allowlist.as_ref() {
            let client_ip = context.client_ip.ok_or_else(|| {
                forbidden(
                    "ip allowlist is enforced but client ip could not be resolved".to_string(),
                )
            })?;
            let allowed = ip_allowed(&client_ip, rules).map_err(|_| {
                internal_server_error(Some("request policy configuration is invalid".to_string()))
            })?;
            if !allowed {
                return Err(forbidden(format!(
                    "client ip {client_ip} is not allowed by relayer policy"
                )));
            }
        }

        if let Some(verification) = permission.request_verification.as_ref() {
            match verify_request_signature(verification, headers) {
                Ok(()) => {}
                Err(SignatureVerificationError::SecretEnvMissing(_))
                | Err(SignatureVerificationError::SecretEnvEmpty(_)) => {
                    return Err(internal_server_error(Some(
                        "request verification is unavailable".to_string(),
                    )));
                }
                Err(error) => return Err(unauthorized(Some(error.to_string()))),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    const RELAYER: &str = "0x1111111111111111111111111111111111111111";

    fn permission(extra: &str) -> NetworkPermissionsConfig {
        serde_yaml::from_str(&format!("relayers: '*'\nallowlist: []\n{extra}")).unwrap()
    }

    fn relayer() -> EvmAddress {
        RELAYER.parse().unwrap()
    }

    #[test]
    fn omitted_controls_preserve_behavior_and_empty_ip_list_denies() {
        assert!(validate_request_policy_entries(
            &[permission("")],
            &PolicyContext::empty(),
            &HeaderMap::new(),
            &relayer(),
        )
        .is_ok());

        let error = validate_request_policy_entries(
            &[permission("ip_allowlist: []\n")],
            &PolicyContext { client_ip: Some("203.0.113.7".parse().unwrap()) },
            &HeaderMap::new(),
            &relayer(),
        )
        .unwrap_err();
        assert_eq!(error.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn ip_runs_before_jwt_and_matching_permission_entries_compose_with_and() {
        let permissions = vec![
            permission("ip_allowlist:\n- 203.0.113.7/32\n"),
            permission("ip_allowlist:\n- 198.51.100.4/32\nrequest_verification:\n  scheme: jwt_hs256\n  params:\n    secret_env: RRELAYER_TEST_POLICY_NEVER_READ\n"),
        ];
        let error = validate_request_policy_entries(
            &permissions,
            &PolicyContext { client_ip: Some("203.0.113.7".parse().unwrap()) },
            &HeaderMap::new(),
            &relayer(),
        )
        .unwrap_err();

        assert_eq!(error.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn missing_jwt_after_allowed_ip_returns_unauthorized() {
        const ENV: &str = "RRELAYER_TEST_POLICY_MISSING_JWT";
        // SAFETY: this test owns a unique environment variable.
        unsafe { std::env::set_var(ENV, "policy-test-secret") };
        let error = validate_request_policy_entries(
            &[permission(&format!(
                "ip_allowlist:\n- 203.0.113.7/32\nrequest_verification:\n  scheme: jwt_hs256\n  params:\n    secret_env: {ENV}\n"
            ))],
            &PolicyContext {
                client_ip: Some("203.0.113.7".parse().unwrap()),
            },
            &HeaderMap::new(),
            &relayer(),
        )
        .unwrap_err();

        assert_eq!(error.0, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn post_start_secret_loss_is_sanitized_and_fails_closed() {
        const ENV: &str = "RRELAYER_TEST_POLICY_REMOVED_AFTER_START";
        // SAFETY: this test owns a unique environment variable.
        unsafe { std::env::remove_var(ENV) };
        let mut headers = HeaderMap::new();
        headers.insert("x-appsmith-signature", "not.a.jwt".parse().unwrap());
        let error = validate_request_policy_entries(
            &[permission(&format!(
                "request_verification:\n  scheme: jwt_hs256\n  params:\n    secret_env: {ENV}\n"
            ))],
            &PolicyContext::empty(),
            &headers,
            &relayer(),
        )
        .unwrap_err();

        assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.1, "request verification is unavailable");
        assert!(!error.1.contains(ENV));
    }
}
