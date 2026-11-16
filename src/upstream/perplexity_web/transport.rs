//! The optional network owner. All production sends retain the fixed consumption-site target.
use super::{Session, ASK_URL, MAX_DEADLINE};
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::{
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
        perplexity_web::{self, failure, Answer, Update},
    },
    routing::candidate::ProviderAccountPayload,
};
use std::time::Duration;

/// The route's resolved model selects the pinned policy; the caller alias is not a wire model.
pub async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let session = Session::from_payload(payload)?;
    let body = perplexity_web::pack_single_turn_for_model(request, model)?;
    // Global transport timeouts can be longer; this product never exceeds its local total budget.
    exchange_body(client, &session, &body, timeout.min(MAX_DEADLINE), ASK_URL)
        .await
        .map(Answer::into_canonical)
}

/// Complete-answer aggregation, not token-by-token forwarding. Dropping the future drops HTTP I/O.
pub async fn execute_single_turn(
    client: &rquest::Client,
    session: &Session,
    request: &CanonicalRelayRequest,
    timeout: Duration,
) -> Result<Answer, GatewayError> {
    exchange(client, session, request, timeout, ASK_URL).await
}

async fn exchange(
    client: &rquest::Client,
    session: &Session,
    request: &CanonicalRelayRequest,
    timeout: Duration,
    url: &str,
) -> Result<Answer, GatewayError> {
    let body = perplexity_web::pack_single_turn(request)?;
    exchange_body(client, session, &body, timeout, url).await
}

async fn exchange_body(
    client: &rquest::Client,
    session: &Session,
    body: &serde_json::Value,
    timeout: Duration,
    url: &str,
) -> Result<Answer, GatewayError> {
    let mut exchange = super::exchange::Exchange::open(client, session, body, timeout, url).await?;
    loop {
        if let Update::Complete(answer) = exchange.next().await? {
            return Ok(answer);
        }
    }
}

pub(super) fn admit_response(response: &rquest::Response) -> Result<(), GatewayError> {
    let status = response.status().as_u16();
    let mut error = match status {
        401 | 403 => {
            let mut e = failure(
                "perplexity_web_authentication",
                "Perplexity Web 会话被拒绝；未切换账号或渠道。",
            );
            e.kind = ErrorKind::Authentication;
            Some(e)
        }
        429 => {
            let mut e = failure(
                "perplexity_web_rate_limit",
                "Perplexity Web 限流；不会自动重提生成。",
            );
            e.kind = ErrorKind::RateLimit;
            Some(e)
        }
        300..=399 => Some(failure(
            "perplexity_web_redirect",
            "Perplexity Web 返回跳转；未跟随或转发 Cookie。",
        )),
        200 => None,
        _ => Some(failure(
            "perplexity_web_http_failed",
            "Perplexity Web HTTP 请求失败；上游正文已省略。",
        )),
    };
    if let Some(error) = error.as_mut() {
        error.http_status = Some(status);
    }
    if let Some(error) = error {
        return Err(error);
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    if media_type.eq_ignore_ascii_case("text/html")
        || media_type.eq_ignore_ascii_case("application/xhtml+xml")
    {
        return Err(failure(
            "perplexity_web_challenge",
            "Perplexity Web 返回 HTML/可能的登录或 challenge 页面，不能作为答案。",
        ));
    }
    if !media_type.eq_ignore_ascii_case("text/event-stream") {
        return Err(failure(
            "perplexity_web_content_type",
            "Perplexity Web 未返回 SSE；响应未按成功处理。",
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > perplexity_web::BODY_LIMIT as u64)
    {
        return Err(failure(
            "perplexity_web_response_limit",
            "Perplexity Web 响应超过本地大小预算。",
        ));
    }
    Ok(())
}
pub(super) fn timeout_error() -> GatewayError {
    let mut error = failure(
        "perplexity_web_timeout",
        "Perplexity Web 总时限已用尽；不会自动重提生成。",
    );
    error.kind = ErrorKind::Timeout;
    error.http_status = Some(504);
    error
}
fn network_error() -> GatewayError {
    let mut error = failure(
        "perplexity_web_network",
        "Perplexity Web 网络失败；私人连接详情已省略，不自动重试。",
    );
    error.kind = ErrorKind::Network;
    error
}

pub(super) fn io_error(error: rquest::Error) -> GatewayError {
    if error.is_timeout() {
        timeout_error()
    } else {
        network_error()
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
