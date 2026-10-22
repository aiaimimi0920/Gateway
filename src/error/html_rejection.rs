//! Recognize non-API upstream diagnostic pages without granting replay admission.
use super::{ErrorKind, FallbackHint, GatewayError};

pub(super) fn classify(status: u16, body: &str, provider: Option<&str>) -> Option<GatewayError> {
    if status != 403 {
        return None;
    }
    // Inspect only a bounded prefix. HTML/script contents must never reach the client.
    let prefix: String = body.chars().take(8192).collect();
    let lower = prefix
        .trim_start_matches('\u{feff}')
        .trim_start()
        .to_lowercase();
    if !lower.starts_with("<!doctype html") && !lower.starts_with("<html") {
        return None;
    }
    let title = lower
        .split_once("<title>")
        .and_then(|(_, tail)| tail.split_once("</title>"))
        .map(|(title, _)| title.split_whitespace().collect::<Vec<_>>().join(" "));
    // Only the observed diagnostic-page signature is eligible. Unknown HTML and
    // explicit policy denials remain terminal, rather than guessing WAF semantics.
    let diagnostic = title.as_deref() == Some("系统终端 - 异常状态诊断")
        && ![
            "content policy",
            "policy violation",
            "permission denied",
            "access denied",
            "content_filter",
            "内容策略",
            "权限不足",
            "禁止访问",
        ]
        .iter()
        .any(|signal| {
            body.as_bytes()
                .windows(signal.len())
                .any(|part| part.eq_ignore_ascii_case(signal.as_bytes()))
        });
    Some(GatewayError {
        kind: if diagnostic {
            ErrorKind::ServiceUnavailable
        } else {
            ErrorKind::Authentication
        },
        message: if diagnostic {
            "Upstream returned an HTML diagnostic page (HTTP 403), not a model API response."
        } else {
            "Upstream returned an HTML access-denied page (HTTP 403)."
        }
        .into(),
        code: Some(
            if diagnostic {
                "upstream_html_diagnostic"
            } else {
                "upstream_html_forbidden"
            }
            .into(),
        ),
        http_status: Some(status),
        retryable: false,
        fallback_hint: if diagnostic {
            FallbackHint::FallbackProvider { reason: "Upstream diagnostic page; try another eligible candidate within the request budget.".into() }
        } else {
            FallbackHint::Abort {
                reason: "Unknown upstream HTML refusal; do not infer replay safety.".into(),
            }
        },
        provider_name: provider.map(str::to_owned),
        upstream_retry_after_ms: None,
    })
}
