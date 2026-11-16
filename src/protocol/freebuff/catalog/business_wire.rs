//! Fixed-path signed transport. No arbitrary headers, redirects, automatic retries or unsigned fallback.
use super::{BusinessError, BusinessReply, BusinessRequests, SessionClaim};
use std::time::Duration;
const SESSION: &str = "/api/v1/freebuff/session";
const ADMISSION: &str = "/api/v1/freebuff/session/admission";
const END_ATTEMPT: &str = "/api/v1/freebuff/session/attempt";
const COMPLETION: &str = "/api/v1/chat/completions";
const REPLY_LIMIT: usize = 1_048_576;
#[derive(Clone, Copy)]
pub(super) struct Authority<'a> {
    pub binding: &'a super::account::Binding,
    pub http: &'a super::transport::CatalogTransport,
    pub device: &'a super::DeviceRegistration,
    pub fetch_id: Option<&'a str>,
}
pub(super) enum Operation<'a> {
    Inspect,
    Heartbeat(&'a SessionClaim),
    Admit(&'a SessionClaim),
    End(&'a SessionClaim),
}
impl Operation<'_> {
    fn target(&self) -> (&'static str, &'static str) {
        match self {
            Self::Inspect | Self::Heartbeat(_) => ("GET", SESSION),
            Self::Admit(_) => ("POST", ADMISSION),
            Self::End(_) => ("DELETE", END_ATTEMPT),
        }
    }
}

#[cfg(feature = "line-freebuff-web-reverse-api")]
fn request(
    scope: Authority<'_>,
    method: &str,
    path: &str,
    bytes: &[u8],
) -> Result<rquest::RequestBuilder, BusinessError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| BusinessError::InvalidInput)?
        .as_millis();
    let timestamp = u64::try_from(timestamp).map_err(|_| BusinessError::InvalidInput)?;
    let mut headers = scope.device.request_headers(
        scope.binding,
        method,
        path,
        bytes,
        scope.fetch_id,
        timestamp,
    )?;
    if let Some(fetch) = scope.fetch_id {
        let mut value = rquest::header::HeaderValue::from_str(fetch).expect("validated fetch id");
        value.set_sensitive(true);
        headers.insert("x-freebuff-catalog-fetch", value);
    }
    Ok(scope
        .http
        .0
        .request(
            rquest::Method::from_bytes(method.as_bytes()).expect("fixed method"),
            format!("{}{path}", scope.binding.transport_origin()),
        )
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .header("authorization", scope.binding.authorization.clone())
        .headers(headers))
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn session(
    scope: Authority<'_>,
    operation: Operation<'_>,
    model_handle: Option<&str>,
    budget: Duration,
) -> Result<BusinessReply, BusinessError> {
    use futures::StreamExt;
    if budget.is_zero() {
        return Err(BusinessError::InvalidInput);
    }
    let budget = budget.min(Duration::from_secs(20));
    tokio::time::timeout(budget, async {
        let (method, path) = operation.target();
        let mut req = request(scope, method, path, b"")?
            .timeout(budget)
            .header("accept", "application/json")
            .header("x-freebuff-catalog-protocol", "1")
            .header("x-freebuff-client", scope.binding.client.as_str());
        let claim = match operation {
            Operation::Inspect => None,
            Operation::Heartbeat(claim) | Operation::Admit(claim) | Operation::End(claim) => {
                Some(claim)
            }
        };
        if let Some(claim) = claim {
            req = req
                .header("x-freebuff-multi-session", "1")
                .header("x-freebuff-purchase-continuity", "1")
                .header("x-freebuff-instance-id", &claim.instance);
            if method == "GET" {
                req = req
                    .header("x-freebuff-heartbeat", "1")
                    .header("x-freebuff-include-unused-rate-limits", "1");
            } else {
                req = req.header("x-freebuff-desktop-attempt-id", &claim.attempt);
            }
        }
        if matches!(operation, Operation::Admit(_)) {
            req = req
                .header(
                    "x-freebuff-model",
                    model_handle.ok_or(BusinessError::InvalidInput)?,
                )
                .header("x-freebuff-wallet-spend-limit", "session");
        }
        let response = req.send().await.map_err(classify)?;
        if response.status().is_redirection() {
            return Err(BusinessError::RedirectRejected);
        }
        if response
            .content_length()
            .is_some_and(|n| n > REPLY_LIMIT as u64)
        {
            return Err(BusinessError::ResponseTooLarge);
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        #[cfg(test)]
        let _ = super::TEST_BODY_STARTED.try_with(|started| started.notify_one());
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(classify)?;
            if chunk.len() > REPLY_LIMIT.saturating_sub(bytes.len()) {
                return Err(BusinessError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| BusinessError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        let body = serde_json::from_slice(&bytes).map_err(|_| BusinessError::InvalidResponse)?;
        Ok(BusinessReply { status, body })
    })
    .await
    .map_err(|_| BusinessError::Timeout)?
}
#[cfg(feature = "line-freebuff-web-reverse-api")]
pub(super) async fn completion(
    scope: &BusinessRequests<'_>,
    bytes: Vec<u8>,
    stream: bool,
    budget: Duration,
) -> Result<rquest::Response, BusinessError> {
    if budget.is_zero() {
        return Err(BusinessError::InvalidInput);
    }
    // Covers the response's lifetime too. The pipeline supplies its request deadline, up to five minutes.
    let budget = budget.min(Duration::from_secs(300));
    tokio::time::timeout(budget, async {
        let response = request(scope.authority(), "POST", COMPLETION, &bytes)?
            .timeout(budget)
            .header(
                "accept",
                if stream {
                    "text/event-stream"
                } else {
                    "application/json"
                },
            )
            .header("content-type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(classify)?;
        if response.status().is_redirection() {
            return Err(BusinessError::RedirectRejected);
        }
        Ok(response)
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
pub(super) async fn session(
    scope: Authority<'_>,
    operation: Operation<'_>,
    model_handle: Option<&str>,
    budget: Duration,
) -> Result<BusinessReply, BusinessError> {
    let _ = (scope, operation, model_handle, budget);
    Err(super::CatalogError::FeatureDisabled.into())
}
#[cfg(not(feature = "line-freebuff-web-reverse-api"))]
pub(super) async fn completion(
    scope: &BusinessRequests<'_>,
    bytes: Vec<u8>,
    stream: bool,
    budget: Duration,
) -> Result<rquest::Response, BusinessError> {
    let _ = (scope, bytes, stream, budget);
    Err(super::CatalogError::FeatureDisabled.into())
}
