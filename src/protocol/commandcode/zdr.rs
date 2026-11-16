//! ZDR is monotonic for a request: a selected account may raise, never lower, privacy.
use super::*;
use std::collections::HashMap;

pub(crate) const HEADER: &str = "x-cmd-zdr";

fn invalid_zdr() -> GatewayError {
    let mut error = GatewayError::bad_request(
        "Command Code ZDR requires x-cmd-zdr: 1 and a compatible route/tool set.",
    )
    .with_code("invalid_commandcode_zdr");
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Do not downgrade the request's ZDR requirement.".into(),
    };
    error
}

pub(crate) fn configured(headers: &HashMap<String, String>) -> Result<bool, GatewayError> {
    if headers.is_empty() {
        return Ok(false);
    }
    // Provider/credential maps merge case-sensitively. Equal requirements may coexist;
    // conflicting values still fail closed, and the sender emits only one header.
    if headers
        .iter()
        .all(|(name, value)| name.eq_ignore_ascii_case(HEADER) && value == "1")
    {
        return Ok(true);
    }
    Err(invalid_zdr())
}

/// Called before candidate admission, including eligibility/capacity skip paths.
/// The context map survives retries/re-routing; do not store this in a single attempt.
pub(crate) fn latch(
    request_headers: &mut HashMap<String, String>,
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    let requested = match request_headers.get(HEADER).map(String::as_str) {
        None => false,
        Some("1") => true,
        _ => return Err(invalid_zdr()),
    };
    if !owns_payload(payload) {
        return if requested {
            Err(invalid_zdr())
        } else {
            Ok(())
        };
    }
    if configured(&payload.headers)? || requested {
        request_headers.insert(HEADER.into(), "1".into());
    }
    Ok(())
}

/// Refresh may rebuild payloads. Reapply the requirement without forwarding caller identity.
pub(crate) fn apply(
    request_headers: &mut HashMap<String, String>,
    payload: &mut ProviderAccountPayload,
) -> Result<(), GatewayError> {
    latch(request_headers, payload)?;
    if request_headers.contains_key(HEADER) {
        payload.headers.clear();
        payload.headers.insert(HEADER.into(), "1".into());
    }
    Ok(())
}

pub(super) fn validate_tools(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Result<(), GatewayError> {
    if !configured(&payload.headers)? || plan.response_kind != EndpointKind::Responses {
        return Ok(());
    }
    // Validate final wire tools, not a lossy canonical projection of native built-ins.
    let tools = &plan.body.as_ref().ok_or_else(invalid_zdr)?["tools"];
    if tools.is_null()
        || tools.as_array().is_some_and(|tools| {
            tools.iter().all(|tool| {
                matches!(
                    tool["type"].as_str(),
                    Some("function" | "custom" | "local_shell")
                )
            })
        })
    {
        Ok(())
    } else {
        Err(invalid_zdr())
    }
}

pub(super) async fn upstream_error(response: rquest::Response) -> GatewayError {
    let status = response.status();
    let mut error = response_error().with_code("commandcode_upstream_rejected");
    error.http_status = Some(if status.is_redirection() {
        502
    } else {
        status.as_u16()
    });
    if status.as_u16() == 422 {
        // Only expose the documented fixed code; never reflect arbitrary upstream text/secrets.
        if let Ok(bytes) = crate::protocol::upstream_body::collect_bounded_upstream_body(
            response,
            "CommandCode ZDR",
        )
        .await
        {
            if serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .is_some_and(|body| {
                    body["error"]["code"] == "cmd_zdr_no_providers"
                        || body["code"] == "cmd_zdr_no_providers"
                })
            {
                error = error.with_code("cmd_zdr_no_providers");
            }
        }
    }
    error
}
