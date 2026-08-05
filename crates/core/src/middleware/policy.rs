use std::net::IpAddr;

use axum::async_trait;
use axum::extract::{ConnectInfo, FromRequestParts, Request};
use axum::http::{request::Parts, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

#[derive(Debug, Clone)]
pub struct PolicyContext {
    pub client_ip: Option<IpAddr>,
}

impl PolicyContext {
    pub fn empty() -> Self {
        Self { client_ip: None }
    }
}

fn extract_client_ip(
    headers: &HeaderMap,
    connect_ip: Option<IpAddr>,
    trust_forwarded_for: bool,
) -> Option<IpAddr> {
    if trust_forwarded_for {
        if let Some(raw) = headers.get("x-forwarded-for") {
            return raw
                .to_str()
                .ok()
                .and_then(|value| value.split(',').next())
                .and_then(|first| first.trim().parse::<IpAddr>().ok());
        }
    }
    connect_ip
}

pub async fn inject_policy_context(
    trust_forwarded_for: bool,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();
    let connect_ip = parts.extensions.get::<ConnectInfo<std::net::SocketAddr>>().map(|v| v.0.ip());
    parts.extensions.insert(PolicyContext {
        client_ip: extract_client_ip(&parts.headers, connect_ip, trust_forwarded_for),
    });
    Ok(next.run(Request::from_parts(parts, body)).await)
}

#[async_trait]
impl<S> FromRequestParts<S> for PolicyContext
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<PolicyContext>().cloned().ok_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::validate_request_policy_entries;
    use crate::shared::HttpError;
    use crate::yaml::NetworkPermissionsConfig;
    use axum::body::Body;
    use axum::http::{HeaderValue, Request, StatusCode};
    use axum::middleware;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn forwarding_resolution_contract_is_explicit() {
        let peer = Some("10.0.0.1".parse().unwrap());
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(" 203.0.113.7, 10.0.0.1"));

        assert_eq!(extract_client_ip(&headers, peer, false), peer);
        assert_eq!(extract_client_ip(&headers, peer, true), Some("203.0.113.7".parse().unwrap()));
        assert_eq!(extract_client_ip(&HeaderMap::new(), peer, true), peer);
    }

    #[test]
    fn malformed_or_non_ascii_trusted_forwarding_is_unresolved() {
        let peer = Some("10.0.0.1".parse().unwrap());
        for value in [
            HeaderValue::from_static("not-an-ip, 203.0.113.7"),
            HeaderValue::from_bytes(&[0xff]).unwrap(),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("x-forwarded-for", value);
            assert_eq!(extract_client_ip(&headers, peer, true), None);
        }
    }

    async fn protected_route(
        context: PolicyContext,
        headers: HeaderMap,
    ) -> Result<StatusCode, HttpError> {
        let permission: NetworkPermissionsConfig =
            serde_yaml::from_str("relayers: '*'\nallowlist: []\nip_allowlist:\n- 203.0.113.7/32\n")
                .unwrap();
        validate_request_policy_entries(
            &[permission],
            &context,
            &headers,
            &"0x1111111111111111111111111111111111111111".parse().unwrap(),
        )?;
        Ok(StatusCode::OK)
    }

    fn request(path: &str, peer: [u8; 4]) -> Request<Body> {
        let mut request = Request::builder().uri(path).body(Body::empty()).unwrap();
        request.extensions_mut().insert(ConnectInfo(std::net::SocketAddr::from((peer, 1234))));
        request
    }

    #[tokio::test]
    async fn route_opt_in_enforces_policy_without_affecting_health() {
        let app = Router::new()
            .route("/protected", get(protected_route))
            .route("/health", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn(|request, next| {
                inject_policy_context(false, request, next)
            }));

        let allowed = app.clone().oneshot(request("/protected", [203, 0, 113, 7])).await.unwrap();
        let disallowed =
            app.clone().oneshot(request("/protected", [198, 51, 100, 4])).await.unwrap();
        let health = app.oneshot(request("/health", [198, 51, 100, 4])).await.unwrap();

        assert_eq!(allowed.status(), StatusCode::OK);
        assert_eq!(disallowed.status(), StatusCode::FORBIDDEN);
        assert_eq!(health.status(), StatusCode::OK);
    }
}
