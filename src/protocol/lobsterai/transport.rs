//! Fixed origins, one decoded-body budget, no retry, redirects or inherited authentication.
use super::LoginError;
use serde_json::Value;
use std::time::Duration;

pub(super) const BUDGET: Duration = Duration::from_secs(15);
pub(super) const TOKEN_LIMIT: usize = 65_536;
pub(super) const CATALOG_LIMIT: usize = 2 * 1024 * 1024;

pub(super) async fn post(
    http: &rquest::Client,
    path: &str,
    body: &Value,
    budget: Duration,
) -> Result<Value, LoginError> {
    if !matches!(path, "/api/auth/exchange" | "/api/auth/refresh") {
        return Err(LoginError::InvalidInput);
    }
    let request = http.post(target(path)).json(body);
    read(request, budget, TOKEN_LIMIT).await
}
pub(super) async fn catalog(
    http: &rquest::Client,
    access: &str,
    client: &super::Client,
    user_id: &str,
) -> Result<Value, LoginError> {
    let mut attribution = client.attribution();
    attribution["userId"] = user_id.into();
    let request = http
        .get(target("/api/models/available"))
        .query(&attribution)
        .bearer_auth(access)
        .header("X-LobsterAI-Client-Version", super::runtime::VERSION)
        .header(
            "X-LobsterAI-Client-Capabilities",
            super::virtual_model::CAPABILITY,
        );
    read(request, BUDGET, CATALOG_LIMIT).await
}
async fn read(
    request: rquest::RequestBuilder,
    budget: Duration,
    limit: usize,
) -> Result<Value, LoginError> {
    #[cfg(test)]
    let budget = TEST_BUDGET
        .try_with(|value| budget.min(*value))
        .unwrap_or(budget);
    tokio::time::timeout(budget, async {
        let response = request.default_headers(false)
            .redirect(rquest::redirect::Policy::none()).timeout(budget)
            .header("accept", "application/json")
            .header("user-agent", concat!("NeuroGateway/", env!("CARGO_PKG_VERSION"), " lobsterai"))
            .send().await.map_err(classify)?;
        let status = response.status();
        if status.is_redirection() { return Err(LoginError::RedirectRejected); }
        if status.as_u16() == 401 { return Err(LoginError::ReauthorizationRequired); }
        if !status.is_success() { return Err(LoginError::Rejected); }
        if response.content_length().is_some_and(|length| length > limit as u64) {
            return Err(LoginError::ResponseTooLarge);
        }
        let bytes = crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(), "LobsterAI response", limit, None
        ).await.map_err(|error| match error.code.as_deref() {
            Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed") => LoginError::ResponseTooLarge,
            _ => LoginError::Transport,
        })?;
        let value = crate::protocol::unique_json::parse(&bytes).map_err(|_| LoginError::InvalidResponse)?;
        if !value.is_object() || value["code"].as_i64() != Some(0) {
            return Err(match value["code"].as_i64() {
                Some(40100 | 40101) => LoginError::ReauthorizationRequired,
                Some(_) => LoginError::Rejected,
                None => LoginError::InvalidResponse,
            });
        }
        Ok(value)
    }).await.map_err(|_| LoginError::Timeout)?
}
fn classify(error: rquest::Error) -> LoginError {
    if error.is_timeout() {
        LoginError::Timeout
    } else {
        LoginError::Transport
    }
}
pub(super) fn target(path: &str) -> String {
    #[cfg(test)]
    {
        let origin = TEST_ORIGIN
            .try_with(Clone::clone)
            .expect("Lobster network tests require an explicit loopback scope");
        let peer = url::Url::parse(&origin).expect("test origin");
        assert_eq!(peer.scheme(), "http");
        assert_eq!(peer.host_str(), Some("127.0.0.1"));
        assert!(peer.port().is_some());
        format!("{origin}{path}")
    }
    #[cfg(not(test))]
    {
        format!("{}{path}", super::API_ORIGIN)
    }
}
#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_ORIGIN: String; pub(super) static TEST_BUDGET: Duration; }
