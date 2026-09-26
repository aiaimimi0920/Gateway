//! Admission errors shared by the local browser refresh workers.

use crate::error::GatewayError;
use crate::upstream::request_time_browser_policy::{
    remote_browser_executor_required_unavailable_error, request_time_browser_forbidden_error,
    RequestTimeBrowserPolicy,
};

pub(super) fn request_time_local_browser_worker_blocking_error(
    policy: RequestTimeBrowserPolicy,
    provider: &str,
) -> Option<GatewayError> {
    match policy {
        RequestTimeBrowserPolicy::Disabled => Some(
            request_time_browser_forbidden_error(
                "Request-time local browser execution is disabled by GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled.",
            )
            .with_provider(provider),
        ),
        RequestTimeBrowserPolicy::RemoteOnly => Some(
            remote_browser_executor_required_unavailable_error(
                "Remote browser executor is required by GATEWAY_REQUEST_TIME_BROWSER_POLICY=remote_only, but keepalive can only launch a local browser worker.",
            )
            .with_provider(provider),
        ),
        RequestTimeBrowserPolicy::LocalAllowed => None,
    }
}
