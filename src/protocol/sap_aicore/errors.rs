//! SAP evidence is deliberately narrower than generic HTTP/body heuristics.
use crate::error::{ErrorKind, GatewayError};
pub(crate) const STREAM_UNVERIFIED: &str = "SAP AI Core stream delivery is indeterminate.";

pub(super) fn interrupted() -> crate::protocol::stream_error::ProtocolStreamError {
    crate::protocol::stream_error::ProtocolStreamError::classified(
        STREAM_UNVERIFIED,
        super::failure("sap_aicore_body_incomplete"),
    )
}

pub(crate) fn rejected(status: u16, retry_after: Option<u64>, oauth: bool) -> GatewayError {
    let mut error = super::failure(if oauth {
        "sap_aicore_token_rejected"
    } else {
        "sap_aicore_upstream_rejected"
    });
    error.http_status = Some(status);
    error.kind = match status {
        401 | 403 => ErrorKind::Authentication,
        429 => ErrorKind::RateLimit,
        400..=499 => ErrorKind::BadRequest,
        _ => ErrorKind::ServerError,
    };
    // The generic builder enables retry. Retain the hint without granting replay.
    error.upstream_retry_after_ms = retry_after;
    error
}

pub(crate) fn http_rejected(response: &rquest::Response, oauth: bool) -> GatewayError {
    rejected(
        response.status().as_u16(),
        crate::error::retry_after_from_headers(response.headers(), std::time::SystemTime::now()),
        oauth,
    )
}

/// OAuth, caller rejection and indeterminate delivery are not inference health evidence.
/// Inference 401/403 does not prove the long-lived Service Key is invalid.
pub(crate) fn neutral(error: &GatewayError) -> bool {
    let Some(code) = error.code.as_deref() else {
        return false;
    };
    if code == "invalid_sap_aicore" {
        return true;
    }
    if !code.starts_with("sap_aicore_") {
        return false;
    }
    match code {
        "sap_aicore_upstream_rejected" => !matches!(error.http_status, Some(429 | 500..=599)),
        "sap_aicore_response_failed" | "sap_aicore_probe_response_failed" => false,
        _ => true,
    }
}

pub(crate) fn upstream_status(error: &GatewayError) -> Option<u16> {
    (error.code.as_deref() == Some("sap_aicore_upstream_rejected"))
        .then_some(error.http_status)
        .flatten()
}
