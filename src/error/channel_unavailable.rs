//! Explicit distributor refusals, not arbitrary model errors or generic 5xx responses.
use super::GatewayError;

pub(crate) fn is_channel_unavailable(error: &GatewayError) -> bool {
    if !matches!(error.http_status, Some(404 | 500 | 502 | 503))
        || !matches!(
            error.code.as_deref(),
            Some("model_not_found" | "upstream_stream_model_not_found" | "get_channel_failed")
        )
    {
        return false;
    }
    let message = error.message.trim();
    // SSE preflight prefixes its structured error code; HTTP errors do not.
    let message = message
        .strip_prefix("model_not_found: ")
        .or_else(|| message.strip_prefix("get_channel_failed: "))
        .unwrap_or(message);
    let message = message.split(" (request id: ").next().unwrap_or(message);
    if let Some(detail) = message
        .strip_prefix("No available channel for model ")
        .and_then(|v| v.strip_suffix(" (distributor)"))
    {
        return detail
            .split_once(" under group ")
            .is_some_and(|(model, group)| !model.trim().is_empty() && !group.trim().is_empty());
    }
    let Some(detail) = message.strip_prefix("分组 ").and_then(|v| {
        v.strip_suffix(" 无可用渠道（distributor）")
            .or_else(|| v.strip_suffix(" 无可用渠道 (distributor)"))
            .or_else(|| v.strip_suffix(" 的可用渠道不存在（retry）"))
    }) else {
        return false;
    };
    detail
        .split_once(" 下模型 ")
        .is_some_and(|(group, model)| !group.trim().is_empty() && !model.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::classify_upstream_error;

    #[test]
    fn only_structured_distributor_refusals_trigger_immediate_cooling() {
        for (status, code, message) in [
            (503, "model_not_found", "No available channel for model deepseek-v4.1-flash under group auto (distributor) (request id: fixture)"),
            (503, "model_not_found", "分组 default 下模型 deepseek-v4.1-flash 无可用渠道（distributor）"),
            (500, "get_channel_failed", "分组 auto 下模型 deepseek-v4.1-flash 的可用渠道不存在（retry） (request id: fixture)"),
        ] {
            let body = serde_json::json!({"error": {"code": code, "message": message}});
            let error = classify_upstream_error(status, &body.to_string(), None);
            assert!(is_channel_unavailable(&error));
            assert!(crate::error::is_explicit_admission_rejection(&error));
            assert!(!error.retryable);
            assert!(matches!(error.fallback_hint, crate::error::FallbackHint::FallbackProvider { .. }));
        }
        for (status, code, message) in [
            (503, "model_not_found", "model_not_found"),
            (
                503,
                "model_not_found",
                "No available channel for model m under group g",
            ),
            (
                503,
                "model_not_found",
                "No available channel for model m under group  (distributor)",
            ),
            (503, "proxy_error", "all proxies are unavailable"),
            (
                403,
                "model_not_found",
                "No available channel for model m under group g (distributor)",
            ),
            (
                503,
                "unknown",
                "No available channel for model m under group g (distributor)",
            ),
        ] {
            let body = serde_json::json!({"error": {"code": code, "message": message}});
            assert!(!is_channel_unavailable(&classify_upstream_error(
                status,
                &body.to_string(),
                None
            )));
        }
    }
}
