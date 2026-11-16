//! Fixed-source START transport is separate from device-signed session/completion wire.
//! No automatic POST retry, acting-user impersonation, caller headers, redirects or guessed FINISH.
use super::{BusinessError, BusinessRequests};
use std::time::Duration;

#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn start(
    scope: &BusinessRequests<'_>,
    budget: Duration,
) -> Result<String, BusinessError> {
    let body = serde_json::to_vec(&serde_json::json!({
        "action":"START", "agentId":"base3-free-catalog", "ancestorRunIds":[]
    }))
    .expect("fixed START envelope");
    let (_, bytes) = post(scope, body, budget).await?;
    let body: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| BusinessError::RunUnconfirmed)?;
    body.get("runId")
        .and_then(serde_json::Value::as_str)
        .filter(|id| super::atom(id, 4096))
        .map(str::to_owned)
        .ok_or(BusinessError::RunUnconfirmed)
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn post(
    scope: &BusinessRequests<'_>,
    body: Vec<u8>,
    budget: Duration,
) -> Result<(u16, Vec<u8>), BusinessError> {
    use futures::StreamExt;
    if budget.is_zero() {
        return Err(BusinessError::InvalidInput);
    }
    let budget = budget.min(Duration::from_secs(20));
    tokio::time::timeout(budget, async {
        let response = scope
            .http
            .0
            .post(format!(
                "{}/api/v1/agent-runs",
                scope.binding.transport_origin()
            ))
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .header("authorization", scope.binding.authorization.clone())
            // Fetch sets this for the SDK's JSON.stringify string body (not a signed JSON API).
            .header("content-type", "text/plain;charset=UTF-8")
            .body(body)
            .timeout(budget)
            .send()
            .await
            .map_err(classify)?;
        if response.status().is_redirection() {
            return Err(BusinessError::RedirectRejected);
        }
        if !response.status().is_success() {
            return Err(BusinessError::RunUnconfirmed);
        }
        let status = response.status().as_u16();
        const LIMIT: usize = 65_536;
        if response.content_length().is_some_and(|n| n > LIMIT as u64) {
            return Err(BusinessError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify)?;
            if chunk.len() > LIMIT.saturating_sub(bytes.len()) {
                return Err(BusinessError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| BusinessError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes))
    })
    .await
    .map_err(|_| BusinessError::Timeout)?
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
fn classify(error: rquest::Error) -> BusinessError {
    if error.is_timeout() {
        BusinessError::Timeout
    } else {
        BusinessError::Transport
    }
}
#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
pub(super) async fn start(
    scope: &BusinessRequests<'_>,
    budget: Duration,
) -> Result<String, BusinessError> {
    let _ = (scope, budget);
    Err(super::CatalogError::FeatureDisabled.into())
}
#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
pub(super) async fn post(
    scope: &BusinessRequests<'_>,
    body: Vec<u8>,
    budget: Duration,
) -> Result<(u16, Vec<u8>), BusinessError> {
    let _ = (scope, body, budget);
    Err(super::CatalogError::FeatureDisabled.into())
}
