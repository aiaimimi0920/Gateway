//! Bounded diagnostics: classify sources without printing URLs, proxy credentials or raw errors.
use super::*;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct NativeTransportEvidence {
    category: &'static str,
    pre_origin_connect: bool,
}

impl NativeTransportEvidence {
    pub fn category(&self) -> &'static str {
        self.category
    }

    pub(crate) fn safe_to_retry(&self) -> bool {
        self.pre_origin_connect
            && !matches!(self.category, "tls_certificate" | "proxy_authentication")
    }
}

impl GatewayError {
    pub(crate) fn provider_response_status(&self) -> Option<u16> {
        if self.native_transport.is_some() || self.request_budget_stop_reason().is_some() {
            None
        } else {
            self.http_status
        }
    }

    pub(crate) fn native_connect_retry_allowed(&self) -> bool {
        self.request_budget_stop_reason().is_none()
            && self.retryable
            && self
                .native_transport
                .as_ref()
                .is_some_and(|e| e.safe_to_retry())
    }
}

pub(crate) fn classify_native_send_error(
    error: reqwest::Error,
    provider: &str,
    no_redirect_observed: bool,
) -> GatewayError {
    // URL/query text must not influence category matching either.
    let error = error.without_url();
    let evidence = evidence(&error, no_redirect_observed);
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Network
    };
    let mut result = GatewayError::service_unavailable(format!(
        "Native transport error: error sending request (category={}, phase={})",
        evidence.category,
        if evidence.pre_origin_connect {
            "pre_origin_connect"
        } else {
            "delivery_unknown"
        },
    ));
    result.kind = kind;
    result.http_status = Some(if error.is_timeout() { 504 } else { 502 });
    result.provider_name = Some(provider.to_owned());
    result.code = Some(format!("native_transport_{}", evidence.category));
    // Unknown delivery is never an implicit replay/failover permission.
    result.retryable = evidence.safe_to_retry();
    result.fallback_hint = FallbackHint::Abort {
        reason: "native_network_failure".into(),
    };
    result.native_transport = Some(evidence);
    result
}

fn evidence(error: &reqwest::Error, no_redirect_observed: bool) -> NativeTransportEvidence {
    let mut category = if error.is_timeout() {
        "timeout"
    } else {
        "send"
    };
    let mut source: Option<&(dyn Error + 'static)> = Some(error);
    for _ in 0..12 {
        let Some(current) = source else { break };
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            category = match io.kind() {
                std::io::ErrorKind::ConnectionRefused => "connection_refused",
                std::io::ErrorKind::ConnectionReset => "connection_reset",
                std::io::ErrorKind::ConnectionAborted => "connection_aborted",
                std::io::ErrorKind::TimedOut => "timeout",
                std::io::ErrorKind::NotConnected => "not_connected",
                _ => category,
            };
        }
        // Examine only a bounded prefix, emit only allowlisted labels, never source text.
        let text: String = current.to_string().chars().take(1024).collect();
        let text = text.to_ascii_lowercase();
        if text.contains("certificate") || text.contains("cert_") {
            category = "tls_certificate";
            break;
        }
        if text.contains("407")
            || text.contains("proxy authentication")
            || text.contains("proxy authorization required")
        {
            category = "proxy_authentication";
            break;
        }
        if text.contains("dns") || text.contains("resolve") || text.contains("getaddrinfo") {
            category = "dns";
        } else if text.contains("tunnel") || text.contains("proxy") {
            category = "proxy_connect";
        } else if text.contains("tls") || text.contains("ssl") || text.contains("handshake") {
            category = "tls_handshake";
        }
        source = current.source();
    }
    NativeTransportEvidence {
        category,
        pre_origin_connect: no_redirect_observed && error.is_connect(),
    }
}
