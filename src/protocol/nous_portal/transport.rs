//! Fixed destinations and one bounded request; no redirect, retry, ambient headers or credential files.
use super::{wire, OAuthError, ReceivedTokens};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;

pub(super) const MAX_RESPONSE: usize = 65_536;
pub(super) const BUDGET: Duration = Duration::from_secs(15);
pub(super) struct Reply {
    pub status: rquest::StatusCode,
    pub value: Option<Value>,
    pub(super) blocked: bool,
}
impl Reply {
    pub fn rejection(&self) -> OAuthError {
        if self.blocked {
            return OAuthError::UpstreamBlocked;
        }
        if matches!(self.status.as_u16(), 408 | 429 | 500..=599) {
            return OAuthError::TemporarilyUnavailable;
        }
        let kind = self
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(Value::as_str);
        match (self.status.as_u16(), kind) {
            (400 | 401 | 403, Some("invalid_grant" | "invalid_token" | "refresh_token_reused")) => {
                OAuthError::ReauthorizationRequired
            }
            (400, Some("access_denied")) => OAuthError::AccessDenied,
            (400, Some("expired_token")) => OAuthError::Expired,
            _ => OAuthError::Rejected,
        }
    }
    pub fn tokens(self) -> Result<ReceivedTokens, OAuthError> {
        if !self.status.is_success() {
            return Err(self.rejection());
        }
        ReceivedTokens::capture(self.value.ok_or(OAuthError::InvalidResponse)?)
    }
}

pub(super) async fn post(
    http: &rquest::Client,
    path: &str,
    form: &[(&str, &str)],
    refresh: Option<&str>,
    budget: Duration,
) -> Result<Reply, OAuthError> {
    if !matches!(path, "/api/oauth/token" | "/api/oauth/device/code") {
        return Err(OAuthError::InvalidInput);
    }
    let mut request = http
        .post(target(&format!("{}{path}", super::PORTAL)))
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(budget)
        .header("accept", "application/json")
        .form(form);
    if let Some(refresh) = refresh {
        request = request.header("x-nous-refresh-token", refresh);
    }
    read(request, budget, MAX_RESPONSE).await
}
pub(super) async fn get_models(
    http: &rquest::Client,
    target_url: &str,
    access: &str,
) -> Result<Value, OAuthError> {
    let request = http
        .get(target(target_url))
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(BUDGET)
        .header("accept", "application/json")
        .bearer_auth(access);
    let reply = read(request, BUDGET, 1_048_576).await?;
    if !reply.status.is_success() {
        return Err(reply.rejection());
    }
    reply.value.ok_or(OAuthError::InvalidResponse)
}
async fn read(
    request: rquest::RequestBuilder,
    budget: Duration,
    limit: usize,
) -> Result<Reply, OAuthError> {
    tokio::time::timeout(budget, async {
        let response = request.send().await.map_err(|_| OAuthError::Transport)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        let blocked = matches!(status.as_u16(), 403 | 429)
            && response.headers().contains_key("x-vercel-mitigated");
        if response
            .content_length()
            .is_some_and(|len| len > limit as u64)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| OAuthError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        let value = wire::object(&bytes).ok();
        Ok(Reply {
            status,
            value,
            blocked,
        })
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}
fn target(url: &str) -> String {
    #[cfg(test)]
    {
        let origin = TEST_ORIGIN
            .try_with(Clone::clone)
            .expect("Nous network tests require an explicit loopback scope");
        let parsed = url::Url::parse(&origin).expect("test origin URL");
        assert_eq!(parsed.scheme(), "http");
        assert_eq!(parsed.host_str(), Some("127.0.0.1"));
        assert!(parsed.port().is_some());
        let url = url::Url::parse(url).expect("fixed production target");
        format!("{origin}{}", url.path())
    }
    #[cfg(not(test))]
    url.into()
}
#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_ORIGIN: String; }
