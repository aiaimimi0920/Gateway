use crate::error::GatewayError;
use rquest::StatusCode;
pub(super) fn provider_quota_http_error(
    label: &str,
    status: StatusCode,
    body: &[u8],
) -> GatewayError {
    let message = String::from_utf8_lossy(body);
    let summary = message.trim();
    let description = if summary.is_empty() {
        format!("{label} quota probe failed with status {status}")
    } else {
        format!(
            "{label} quota probe failed with status {status}: {}",
            summary.chars().take(240).collect::<String>()
        )
    };
    if status.as_u16() >= 500 {
        GatewayError::service_unavailable(description).with_code("provider_quota_http_error")
    } else {
        GatewayError::conflict(description).with_code("provider_quota_http_error")
    }
}

pub(super) fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}
