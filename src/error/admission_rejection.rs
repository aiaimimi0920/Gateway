//! Narrow upstream refusals eligible for candidate switching, never arbitrary 4xx/5xx replay.
use super::GatewayError;

pub(crate) fn is_account_balance_rejection(status: u16, message: &str) -> bool {
    let message = message.trim();
    // This is an admission refusal before generation, not an arbitrary 403.
    // Amounts and the optional request-id suffix vary between requests.
    matches!(status, 402 | 403)
        && (message.eq_ignore_ascii_case("Insufficient account balance")
            || message
                .strip_prefix("预扣费额度失败")
                .is_some_and(|details| {
                    details.contains("用户剩余额度:") && details.contains("需要预扣费额度:")
                })
            || message
                .strip_prefix("Not enough quota to pre-consume for this request:")
                .is_some_and(|details| {
                    details.contains(" remaining, ") && details.contains(" required")
                }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const IP_BLOCK: &str = "该ip已被封禁，原因：bulk probe guard: ip 27.47.35.168 requested 5 distinct models in 60s (last_user 11552, offense 1)";

    #[test]
    fn bulk_probe_ip_block_is_a_narrow_retryable_provider_admission_refusal() {
        assert!(is_provider_ip_block_rejection(403, IP_BLOCK));
        assert!(!is_provider_ip_block_rejection(401, IP_BLOCK));
        assert!(!is_provider_ip_block_rejection(
            403,
            "该ip已被封禁，原因：invalid api key"
        ));
        assert!(!is_provider_ip_block_rejection(
            403,
            "该ip已被封禁，原因：bulk probe guard: ip not-an-ip requested 5 distinct models in 60s (last_user 11552, offense 1)"
        ));
        assert!(!is_provider_ip_block_rejection(
            403,
            "Forbidden by content policy"
        ));
    }

    #[test]
    fn bulk_probe_ip_block_code_alone_cannot_authorize_replay() {
        for status in [400, 403, 500] {
            let error = super::super::classify_upstream_error(
                status,
                r#"{"error":{"code":"provider_ip_temporarily_blocked","message":"unrelated provider failure"}}"#,
                None,
            );
            assert!(!is_explicit_admission_rejection(&error));
        }
    }

    #[test]
    fn bulk_probe_ip_block_plain_text_preserves_provider_scope_evidence() {
        let error = super::super::classify_upstream_error(403, IP_BLOCK, None);
        assert_eq!(error.message, IP_BLOCK);
        assert!(is_explicit_admission_rejection(&error));
    }
}

pub(crate) fn is_provider_ip_block_rejection(status: u16, message: &str) -> bool {
    if status != 403 || message.len() > 512 {
        return false;
    }
    let Some(details) = message
        .trim()
        .strip_prefix("该ip已被封禁，原因：bulk probe guard: ip ")
    else {
        return false;
    };
    let Some((ip, evidence)) = details.split_once(" requested ") else {
        return false;
    };
    if ip.parse::<std::net::IpAddr>().is_err() {
        return false;
    }
    let mut fields = evidence.split_ascii_whitespace();
    fields
        .next()
        .is_some_and(|count| count.parse::<u32>().is_ok_and(|count| count > 0))
        && fields.next() == Some("distinct")
        && fields.next() == Some("models")
        && fields.next() == Some("in")
        && fields
            .next()
            .and_then(|window| window.strip_suffix('s'))
            .is_some_and(|seconds| seconds.parse::<u32>().is_ok_and(|seconds| seconds > 0))
        && fields.next() == Some("(last_user")
        && fields
            .next()
            .and_then(|user| user.strip_suffix(','))
            .is_some_and(|user| user.parse::<u64>().is_ok())
        && fields.next() == Some("offense")
        && fields
            .next()
            .and_then(|offense| offense.strip_suffix(')'))
            .is_some_and(|offense| offense.parse::<u32>().is_ok_and(|n| n > 0))
        && fields.next().is_none()
}

pub(crate) fn is_provider_ip_block(error: &GatewayError) -> bool {
    // Upstream JSON may supply arbitrary codes; code alone is not trusted refusal evidence.
    error.kind == super::ErrorKind::ServiceUnavailable
        && error.code.as_deref() == Some("provider_ip_temporarily_blocked")
        && matches!(
            error.fallback_hint,
            super::FallbackHint::FallbackProvider { .. }
        )
        && error
            .http_status
            .is_some_and(|status| is_provider_ip_block_rejection(status, &error.message))
}

pub(crate) fn is_explicit_admission_rejection(error: &GatewayError) -> bool {
    // Local authentication errors retain Abort and cannot acquire a replay slot.
    (error.http_status == Some(401)
        && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        || (error.http_status == Some(404)
            && error.code.as_deref() == Some("upstream_stream_model_not_found"))
        // A relay may wrap a structured pre-stream model refusal in HTTP 502.
        || (error.http_status == Some(502)
            && error.kind == super::ErrorKind::ModelNotFound
            && error.code.as_deref() == Some("upstream_stream_initialization_model_not_found")
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        // The recognized non-API diagnostic page is an upstream invocation failure.
        // Only its trusted classification may advance a tool conversation before handoff;
        // a JSON error borrowing the code, unknown HTML or policy denial remains closed.
        || (error.http_status == Some(403)
            && error.kind == super::ErrorKind::ServiceUnavailable
            && error.code.as_deref() == Some("upstream_html_diagnostic")
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        || (error.kind == super::ErrorKind::InsufficientQuota
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. })
            && error.http_status.is_some_and(|status| is_account_balance_rejection(status, &error.message)))
        // A structured overload admission refusal is safe to advance; a generic
        // 503 or a transport failure does not prove that execution never started.
        || (error.http_status == Some(503)
            && error.code.as_deref() == Some("system_cpu_overloaded"))
        || is_provider_ip_block(error)
        || super::is_channel_unavailable(error)
        || super::model_pricing::is_admission_rejection(error)
}
