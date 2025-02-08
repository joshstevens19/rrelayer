#![allow(unused_imports)]

use axum::{
    async_trait,
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::authentication::{
    jwt::validate_token_includes_role,
    types::{JwtRole, JwtTokenType},
};

pub enum JwtTokenOrApiKeyGuardResult {
    JwtToken(String),
    ApiKey(String),
}

impl JwtTokenOrApiKeyGuardResult {
    pub fn is_api_key(&self) -> bool {
        matches!(self, JwtTokenOrApiKeyGuardResult::ApiKey(_))
    }
}

fn validate_auth_token(
    headers: &HeaderMap,
    jwt_token_type: JwtTokenType,
    jwt_role: Vec<JwtRole>,
) -> Result<String, StatusCode> {
    headers
        .get("Authorization")
        .and_then(|token| token.to_str().ok())
        .and_then(|token| token.strip_prefix("Bearer "))
        .and_then(|token| {
            validate_token_includes_role(jwt_token_type, jwt_role, token)
                .ok()
                .map(|_| token.to_string())
        })
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn extract_api_key(headers: &HeaderMap) -> Result<String, StatusCode> {
    headers
        .get("x-api-key")
        .and_then(|api_key| api_key.to_str().ok())
        .map(String::from)
        .ok_or(StatusCode::UNAUTHORIZED)
}

macro_rules! impl_jwt_guard {
    ($name:ident, $guard_fn_name:ident, $roles:expr) => {
        pub struct $name(pub String);

        #[async_trait]
        impl<S> FromRequestParts<S> for $name
        where
            S: Send + Sync,
        {
            type Rejection = StatusCode;

            async fn from_request_parts(
                parts: &mut Parts,
                _state: &S,
            ) -> Result<Self, Self::Rejection> {
                validate_auth_token(&parts.headers, JwtTokenType::Access, $roles.to_vec())
                    .map($name)
                    .map_err(|_| StatusCode::UNAUTHORIZED)
            }
        }

        pub async fn $guard_fn_name(
            req: Request<Body>,
            next: Next,
        ) -> Result<Response<Body>, StatusCode> {
            let (mut parts, body) = req.into_parts();
            $name::from_request_parts(&mut parts, &()).await?;
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }
    };
}

impl_jwt_guard!(AdminJwtTokenGuard, admin_jwt_guard, [JwtRole::Admin]);
impl_jwt_guard!(ManagerJwtTokenGuard, manager_jwt_guard, [JwtRole::Manager]);
impl_jwt_guard!(
    ManagerOrAboveJwtTokenGuard,
    manager_or_above_jwt_guard,
    [JwtRole::Admin, JwtRole::Manager]
);
impl_jwt_guard!(IntegratorJwtTokenGuard, integrator_jwt_guard, [JwtRole::Integrator]);
impl_jwt_guard!(
    IntegratorOrAboveJwtTokenGuard,
    integrator_or_above_jwt_guard,
    [JwtRole::Integrator, JwtRole::Admin, JwtRole::Manager]
);
impl_jwt_guard!(ReadOnlyJwtTokenGuard, read_only_jwt_guard, [JwtRole::ReadOnly]);
impl_jwt_guard!(
    ReadOnlyOrAboveJwtTokenGuard,
    read_only_or_above_jwt_guard,
    [JwtRole::Admin, JwtRole::Manager, JwtRole::Integrator, JwtRole::ReadOnly]
);

pub struct ManagerOrAboveJwtTokenOrApiKeyGuard(pub JwtTokenOrApiKeyGuardResult);

impl ManagerOrAboveJwtTokenOrApiKeyGuard {
    pub fn is_api_key(&self) -> bool {
        self.0.is_api_key()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ManagerOrAboveJwtTokenOrApiKeyGuard
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        validate_auth_token(
            &parts.headers,
            JwtTokenType::Access,
            vec![JwtRole::Admin, JwtRole::Manager],
        )
        .map(|token| Self(JwtTokenOrApiKeyGuardResult::JwtToken(token)))
        .or_else(|_| {
            extract_api_key(&parts.headers)
                .map(|api_key| Self(JwtTokenOrApiKeyGuardResult::ApiKey(api_key)))
        })
    }
}

pub async fn manager_or_above_jwt_or_api_key_guard(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();
    ManagerOrAboveJwtTokenOrApiKeyGuard::from_request_parts(&mut parts, &()).await?;
    req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}

pub struct ReadOnlyOrAboveJwtTokenOrApiKeyGuard(pub JwtTokenOrApiKeyGuardResult);

impl ReadOnlyOrAboveJwtTokenOrApiKeyGuard {
    pub fn is_api_key(&self) -> bool {
        self.0.is_api_key()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ReadOnlyOrAboveJwtTokenOrApiKeyGuard
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        validate_auth_token(
            &parts.headers,
            JwtTokenType::Access,
            vec![JwtRole::Admin, JwtRole::Manager, JwtRole::Integrator, JwtRole::ReadOnly],
        )
        .map(|token| Self(JwtTokenOrApiKeyGuardResult::JwtToken(token)))
        .or_else(|_| {
            extract_api_key(&parts.headers)
                .map(|api_key| Self(JwtTokenOrApiKeyGuardResult::ApiKey(api_key)))
        })
    }
}

pub async fn read_only_or_above_jwt_or_api_key_guard(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();
    ReadOnlyOrAboveJwtTokenOrApiKeyGuard::from_request_parts(&mut parts, &()).await?;
    req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}

pub struct RefreshJwtTokenGuard(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for RefreshJwtTokenGuard
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        validate_auth_token(
            &parts.headers,
            JwtTokenType::Refresh,
            vec![JwtRole::Admin, JwtRole::Manager, JwtRole::Integrator, JwtRole::ReadOnly],
        )
        .map(RefreshJwtTokenGuard)
    }
}

pub async fn refresh_jwt_token_guard(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();
    RefreshJwtTokenGuard::from_request_parts(&mut parts, &()).await?;
    req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}
