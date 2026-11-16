//! Direct Web metadata only: no redirects/replay, bounded decoded JSON, one shared deadline.
use crate::{
    error::{ErrorKind, FallbackHint, GatewayError},
    upstream::{
        client::UpstreamClient,
        common::RequestPlan,
        public_assets,
        suno_response_helpers::classify_suno_upstream_error,
        suno_runtime_helpers::{build_suno_runtime_headers, PreparedSunoExecutionContext},
    },
};
use serde_json::Value;

pub(super) const TOTAL_LIMIT: usize = 1024 * 1024;
const RESPONSE_LIMIT: usize = 128 * 1024;

pub(super) fn no_replay(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Suno generation must not be replayed; remote credits may already be spent.".into(),
    };
    error
}
pub(super) fn failure(code: &'static str, message: &str) -> GatewayError {
    let mut error = GatewayError::server_error(message)
        .with_provider("suno_compatible")
        .with_code(code);
    error.http_status = Some(502);
    no_replay(error)
}
pub(super) fn timeout() -> GatewayError {
    let mut error = failure(
        "suno_timeout",
        "Suno 总时限已用尽；停止等待不撤回远端生成，不自动重提或换号。",
    );
    error.kind = ErrorKind::Timeout;
    error.http_status = Some(504);
    error
}
fn read_error(error: public_assets::Failure) -> GatewayError {
    match error {
        public_assets::Failure::Timeout => timeout(),
        public_assets::Failure::Limit => failure("suno_body_limit", "Suno 响应超过解码字节预算。"),
        _ => failure("suno_network", "Suno 响应读取失败；不自动重提。"),
    }
}
pub(super) async fn send(
    client: &UpstreamClient,
    plan: &RequestPlan,
    context: &PreparedSunoExecutionContext,
    deadline: tokio::time::Instant,
    total: &mut usize,
) -> Result<Value, GatewayError> {
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    if remaining.is_zero() {
        return Err(timeout());
    }
    // Keep the prepared device identity, but refresh the timestamp for every request.
    let headers = build_suno_runtime_headers(&context.runtime_headers);
    let response = client
        .send_plan(plan, headers)
        .redirect(rquest::redirect::Policy::none())
        .timeout(remaining)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                timeout()
            } else {
                let mut error = failure(
                    "suno_network",
                    "Suno 网络失败；远端操作可能已发生，不自动重提。",
                );
                error.kind = ErrorKind::Network;
                error
            }
        })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        // An error document is not an authorized diagnostic/secret disclosure surface.
        let mut error = classify_suno_upstream_error(
            status,
            response.headers(),
            "Suno 请求失败；上游正文已省略，不自动重提。",
            context.execution_plan.missing_challenge_token,
        );
        if !(400..=599).contains(&status) {
            error.http_status = Some(502);
        }
        return Err(no_replay(error));
    }
    let json = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(failure(
            "suno_content_type",
            "Suno 未返回 JSON；正文已省略。",
        ));
    }
    let limit = TOTAL_LIMIT.saturating_sub(*total).min(RESPONSE_LIMIT);
    let bytes = public_assets::read(response, limit)
        .await
        .map_err(read_error)?;
    *total += bytes.len(); // read() proved bytes.len() <= TOTAL_LIMIT - total.
    serde_json::from_slice(&bytes)
        .map_err(|_| failure("suno_invalid_response", "Suno JSON 无效；正文已省略。"))
}
