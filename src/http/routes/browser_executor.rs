use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::access_control::{
    authorize_internal_request, unauthenticated_browser_executor_allowed, InternalAccessSurface,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use crate::upstream::client::{
    BrowserExecutorServiceHealth, BrowserExecutorServiceInvocationRequest,
    BrowserExecutorServiceInvocationResponse,
};

pub async fn get_browser_executor_health(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<BrowserExecutorServiceHealth>, GatewayError> {
    assert_browser_executor_access(token.as_deref(), &headers)?;
    Ok(Json(
        state.upstream_client.browser_executor_runtime_health(),
    ))
}

pub async fn execute_browser_executor(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<BrowserExecutorServiceInvocationRequest>,
) -> Result<Json<BrowserExecutorServiceInvocationResponse>, GatewayError> {
    assert_browser_executor_access(token.as_deref(), &headers)?;
    let response = state
        .upstream_client
        .execute_browser_executor_service_invocation(body)
        .await;
    Ok(Json(response))
}

fn assert_browser_executor_access(
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    let expected = std::env::var("GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    assert_browser_executor_access_with_expected(
        expected.as_deref(),
        unauthenticated_browser_executor_allowed(),
        bearer_token,
        headers,
    )
}

fn assert_browser_executor_access_with_expected(
    expected: Option<&str>,
    allow_unauthenticated: bool,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    authorize_internal_request(
        InternalAccessSurface::BrowserExecutor,
        expected,
        allow_unauthenticated,
        bearer_token,
        headers,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_map(name: &'static str, value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, value.parse().expect("valid header value"));
        headers
    }

    #[test]
    fn browser_executor_access_rejects_missing_config_by_default() {
        let err =
            assert_browser_executor_access_with_expected(None, false, None, &HeaderMap::new())
                .expect_err("missing browser executor token must fail closed");

        assert_eq!(err.http_status, Some(503));
        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_token_not_configured")
        );
    }

    #[test]
    fn browser_executor_access_allows_missing_config_only_with_explicit_override() {
        assert!(
            assert_browser_executor_access_with_expected(None, true, None, &HeaderMap::new())
                .is_ok()
        );
    }

    #[test]
    fn browser_executor_access_accepts_header_and_bearer_tokens() {
        assert!(assert_browser_executor_access_with_expected(
            Some("secret"),
            false,
            None,
            &header_map("x-internal-api-key", "secret")
        )
        .is_ok());

        assert!(assert_browser_executor_access_with_expected(
            Some("secret"),
            false,
            Some("secret"),
            &HeaderMap::new()
        )
        .is_ok());
    }

    #[test]
    fn browser_executor_access_rejects_wrong_token() {
        let err = assert_browser_executor_access_with_expected(
            Some("secret"),
            false,
            Some("wrong"),
            &HeaderMap::new(),
        )
        .expect_err("wrong browser executor token must be rejected");

        assert_eq!(err.http_status, Some(401));
    }
}
