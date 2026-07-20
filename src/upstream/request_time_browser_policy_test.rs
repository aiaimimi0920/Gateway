use crate::upstream::request_time_browser_policy::{
    remote_browser_executor_required_failed_error,
    remote_browser_executor_required_unavailable_error, request_time_browser_forbidden_error,
    RequestTimeBrowserPolicy,
};

#[test]
fn parses_request_time_browser_policy_values() {
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(None),
        RequestTimeBrowserPolicy::LocalAllowed
    );
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(Some(" disabled ")),
        RequestTimeBrowserPolicy::Disabled
    );
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(Some("remote-only")),
        RequestTimeBrowserPolicy::RemoteOnly
    );
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(Some("remote_only")),
        RequestTimeBrowserPolicy::RemoteOnly
    );
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(Some("local-allowed")),
        RequestTimeBrowserPolicy::LocalAllowed
    );
    assert_eq!(
        RequestTimeBrowserPolicy::from_optional_value(Some("unexpected")),
        RequestTimeBrowserPolicy::LocalAllowed
    );
}

#[test]
fn request_time_browser_policy_reports_local_fallback_blocking() {
    assert!(RequestTimeBrowserPolicy::Disabled.forbids_local_fallback());
    assert!(RequestTimeBrowserPolicy::RemoteOnly.forbids_local_fallback());
    assert!(!RequestTimeBrowserPolicy::LocalAllowed.forbids_local_fallback());
}

#[test]
fn request_time_browser_policy_error_helpers_keep_existing_codes() {
    assert_eq!(
        remote_browser_executor_required_unavailable_error("missing")
            .code
            .as_deref(),
        Some("browser_executor_required_unavailable")
    );
    assert_eq!(
        remote_browser_executor_required_failed_error("failed")
            .code
            .as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(
        request_time_browser_forbidden_error("forbidden")
            .code
            .as_deref(),
        Some("request_time_browser_forbidden")
    );
}
