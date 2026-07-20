use crate::error::GatewayError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestTimeBrowserPolicy {
    Disabled,
    RemoteOnly,
    LocalAllowed,
}

impl RequestTimeBrowserPolicy {
    pub(crate) fn from_env() -> Self {
        Self::from_optional_value(
            std::env::var("GATEWAY_REQUEST_TIME_BROWSER_POLICY")
                .ok()
                .as_deref(),
        )
    }

    pub(crate) fn from_optional_value(value: Option<&str>) -> Self {
        match value
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("disabled") => Self::Disabled,
            Some("remote_only") | Some("remote-only") => Self::RemoteOnly,
            Some("local_allowed") | Some("local-allowed") => Self::LocalAllowed,
            _ => Self::LocalAllowed,
        }
    }

    pub(crate) fn forbids_local_fallback(self) -> bool {
        matches!(self, Self::Disabled | Self::RemoteOnly)
    }
}

pub(crate) fn remote_browser_executor_required_unavailable_error(
    message: impl Into<String>,
) -> GatewayError {
    GatewayError::service_unavailable(message).with_code("browser_executor_required_unavailable")
}

pub(crate) fn remote_browser_executor_required_failed_error(
    message: impl Into<String>,
) -> GatewayError {
    GatewayError::service_unavailable(message).with_code("browser_executor_required_failed")
}

pub(crate) fn request_time_browser_forbidden_error(message: impl Into<String>) -> GatewayError {
    GatewayError::service_unavailable(message).with_code("request_time_browser_forbidden")
}
