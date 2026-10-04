//! Closed label domains: never export an error message, request identifier or URL.
#[derive(Debug, Clone, Copy)]
pub enum RequestTermination {
    Completed,
    Cancelled,
    Interrupted,
}

impl RequestTermination {
    pub(super) fn index(self) -> usize {
        self as usize
    }
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "stream_interrupted",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ReliabilityEvent {
    Retry,
    Fallback,
}

impl ReliabilityEvent {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Fallback => "fallback",
        }
    }
}

pub(super) fn bounded_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("credential_invalid") => "credential_invalid",
        Some("credential_expired") => "credential_expired",
        Some("quota_exhausted") => "quota_exhausted",
        Some("rate_limited") => "rate_limited",
        Some("model_unsupported") => "model_unsupported",
        Some("content_rejected") => "content_rejected",
        Some("gateway_protocol_error") => "gateway_protocol_error",
        Some("client_request_invalid") => "client_request_invalid",
        Some("provider_transient") => "provider_transient",
        Some("cancelled") => "cancelled",
        Some("stream_interrupted") => "stream_interrupted",
        Some("same_candidate") => "same_candidate",
        Some("recovery") => "recovery",
        _ => "unknown",
    }
}
