//! No retry/redirect; one deadline and cumulative decoded metadata budget own every step.
use super::{admission::Body, failure, no_replay, parse_chataibot_http_response};
use crate::{
    error::{ErrorKind, GatewayError},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::Value;

pub(super) async fn send(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    headers: &HeaderMap,
    path: &str,
    body: Option<Body>,
    deadline: tokio::time::Instant,
    total: &mut usize,
    json: bool,
) -> Result<Value, GatewayError> {
    let url = format!("{}{path}", payload.base_url.trim_end_matches('/'));
    #[cfg(test)]
    let url = super::test_support::target(&url);
    let mut headers = headers.clone();
    headers.insert(
        "x-supports-async-image-jobs",
        HeaderValue::from_static("true"),
    );
    let request = match body {
        Some(Body::Json(value)) => {
            headers.insert("content-type", HeaderValue::from_static("application/json"));
            client.post(&url).json(&value)
        }
        Some(Body::Multipart(body)) => {
            headers.insert(
                "content-type",
                HeaderValue::from_str(&body.content_type).expect("locally generated boundary"),
            );
            client.post(&url).body(body.bytes)
        }
        None => client.get(&url),
    };
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return Err(timeout());
    }
    let response = request
        .default_headers(false)
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .timeout(remaining)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                timeout()
            } else {
                let mut error = failure(
                    "chataibot_network",
                    "ChatAIBot 网络失败；远端操作可能已发生，不自动重提。",
                );
                error.kind = ErrorKind::Network;
                error
            }
        })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let mut error = crate::error::classify_upstream_error(
            status,
            "ChatAIBot 请求失败；上游正文已省略，不自动重提。",
            Some("chataibot_compatible"),
        );
        if !(400..=599).contains(&status) {
            error.http_status = Some(502);
        }
        return Err(no_replay(error));
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = super::super::public_assets::read(response, 128 * 1024)
        .await
        .map_err(super::assets::error)?;
    *total = total
        .checked_add(bytes.len())
        .filter(|n| *n <= 1024 * 1024)
        .ok_or_else(|| failure("chataibot_body_limit", "ChatAIBot 响应超过累计预算。"))?;
    if !json {
        return Ok(Value::Null);
    }
    if !mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
    {
        return Err(failure(
            "chataibot_content_type",
            "ChatAIBot 未返回 JSON；正文已省略。",
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| failure("chataibot_invalid_response", "ChatAIBot JSON 编码无效。"))?;
    parse_chataibot_http_response(status, Some(&mime), text, "chataibot_compatible")
}
pub(super) fn timeout() -> GatewayError {
    let mut error = failure(
        "chataibot_timeout",
        "ChatAIBot 总时限已用尽；本地停止等待不撤回远端生成，不自动重提。",
    );
    error.kind = ErrorKind::Timeout;
    error.http_status = Some(504);
    error
}
