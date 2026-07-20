use crate::protocol::udio;
use crate::upstream::browser_executor_helpers::{
    build_browser_executor_header_map, missing_browser_executor_field_error, read_json_bool,
    read_json_u64,
};
use crate::upstream::browser_worker_types::UdioBrowserWorkerInput;
use crate::upstream::header_map_helpers::header_map_string;
use rquest::header::HeaderMap;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedUdioBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) runtime_state_object_key: Option<String>,
    pub(crate) headers: HeaderMap,
    pub(crate) request_body: Value,
    pub(crate) target_asset_kind: udio::UdioOutputKind,
    pub(crate) wait_audio: bool,
    pub(crate) wait_timeout: Duration,
    pub(crate) poll_interval: Duration,
    pub(crate) timeout: Duration,
}

pub(crate) fn build_udio_browser_worker_input<'a>(
    base_url: &'a str,
    runtime_state_object_key: Option<&str>,
    headers: &HeaderMap,
    request_body: &'a Value,
    target_asset_kind: udio::UdioOutputKind,
    wait_audio: bool,
    wait_timeout: Duration,
    poll_interval: Duration,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> UdioBrowserWorkerInput<'a> {
    UdioBrowserWorkerInput {
        base_url,
        cookie_header: header_map_string(headers, "cookie"),
        request_body,
        target_asset_kind: target_asset_kind.browser_target_key(),
        wait_audio,
        wait_timeout_ms: wait_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        poll_interval_ms: poll_interval.as_millis().min(u128::from(u64::MAX)) as u64,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        runtime_state_object_key: runtime_state_object_key.map(str::to_string),
        browser_executable_path,
        user_agent: header_map_string(headers, "user-agent"),
        accept_language: header_map_string(headers, "accept-language"),
        origin: header_map_string(headers, "origin"),
        referer: header_map_string(headers, "referer"),
    }
}

pub(crate) fn build_udio_browser_executor_payload(
    base_url: &str,
    runtime_state_object_key: Option<&str>,
    headers: &HeaderMap,
    request_body: &Value,
    target_asset_kind: udio::UdioOutputKind,
    wait_audio: bool,
    wait_timeout: Duration,
    poll_interval: Duration,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> serde_json::Value {
    serde_json::json!({
        "baseUrl": base_url,
        "cookieHeader": header_map_string(headers, "cookie"),
        "requestBody": request_body,
        "targetAssetKind": target_asset_kind.browser_target_key(),
        "waitAudio": wait_audio,
        "waitTimeoutMs": wait_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "pollIntervalMs": poll_interval.as_millis().min(u128::from(u64::MAX)) as u64,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "runtimeStateObjectKey": runtime_state_object_key,
        "browserExecutablePath": browser_executable_path,
        "userAgent": header_map_string(headers, "user-agent"),
        "acceptLanguage": header_map_string(headers, "accept-language"),
        "origin": header_map_string(headers, "origin"),
        "referer": header_map_string(headers, "referer"),
    })
}

pub(crate) fn prepare_udio_browser_executor_service_input(
    input: &Value,
) -> Result<PreparedUdioBrowserExecutorServiceInput, crate::error::GatewayError> {
    let base_url = input
        .get("baseUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "Udio",
                "baseUrl",
                "browser_executor_missing_base_url",
            )
        })?;
    let request_body = input.get("requestBody").cloned().ok_or_else(|| {
        missing_browser_executor_field_error(
            "Udio",
            "requestBody",
            "browser_executor_missing_request_body",
        )
    })?;
    let target_asset_kind = match input
        .get("targetAssetKind")
        .and_then(|value| value.as_str())
        .map(str::trim)
    {
        Some("image") => udio::UdioOutputKind::Image,
        Some("audio") => udio::UdioOutputKind::Music,
        Some("video") => udio::UdioOutputKind::Video,
        _ => udio::UdioOutputKind::Music,
    };

    Ok(PreparedUdioBrowserExecutorServiceInput {
        base_url,
        runtime_state_object_key: input
            .get("runtimeStateObjectKey")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        headers: build_browser_executor_header_map(input),
        request_body,
        target_asset_kind,
        wait_audio: read_json_bool(input, "waitAudio").unwrap_or(true),
        wait_timeout: Duration::from_millis(
            read_json_u64(input, "waitTimeoutMs").unwrap_or(180_000),
        ),
        poll_interval: Duration::from_millis(
            read_json_u64(input, "pollIntervalMs").unwrap_or(3_000),
        ),
        timeout: Duration::from_millis(read_json_u64(input, "timeoutMs").unwrap_or(300_000)),
    })
}

#[cfg(test)]
mod tests {
    use crate::protocol::udio;
    use rquest::header::HeaderMap;
    use serde_json::json;

    use super::{
        build_udio_browser_executor_payload, build_udio_browser_worker_input,
        prepare_udio_browser_executor_service_input,
    };

    #[test]
    fn build_udio_browser_worker_input_preserves_runtime_and_wait_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "udio-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.udio.com".parse().unwrap());
        headers.insert("referer", "https://www.udio.com/create".parse().unwrap());
        let request_body = json!({ "prompt": "cinematic chorus" });

        let input = build_udio_browser_worker_input(
            "https://www.udio.com",
            Some("runtime-123"),
            &headers,
            &request_body,
            udio::UdioOutputKind::Music,
            true,
            std::time::Duration::from_secs(480),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );
        let payload = serde_json::to_value(&input).expect("udio worker input");

        assert_eq!(payload["baseUrl"], "https://www.udio.com");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cinematic chorus");
        assert_eq!(payload["targetAssetKind"], "audio");
        assert_eq!(payload["waitAudio"], true);
        assert_eq!(payload["waitTimeoutMs"], 480_000u64);
        assert_eq!(payload["pollIntervalMs"], 5_000u64);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["runtimeStateObjectKey"], "runtime-123");
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "udio-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.udio.com");
        assert_eq!(payload["referer"], "https://www.udio.com/create");
    }

    #[test]
    fn build_udio_browser_executor_payload_preserves_runtime_and_wait_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "udio-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.udio.com".parse().unwrap());
        headers.insert("referer", "https://www.udio.com/create".parse().unwrap());
        let request_body = json!({ "prompt": "cinematic chorus" });

        let payload = build_udio_browser_executor_payload(
            "https://www.udio.com",
            Some("runtime-123"),
            &headers,
            &request_body,
            udio::UdioOutputKind::Music,
            true,
            std::time::Duration::from_secs(480),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://www.udio.com");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cinematic chorus");
        assert_eq!(payload["targetAssetKind"], "audio");
        assert_eq!(payload["waitAudio"], true);
        assert_eq!(payload["waitTimeoutMs"], 480_000u64);
        assert_eq!(payload["pollIntervalMs"], 5_000u64);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["runtimeStateObjectKey"], "runtime-123");
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "udio-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.udio.com");
        assert_eq!(payload["referer"], "https://www.udio.com/create");
    }

    #[test]
    fn prepare_udio_browser_executor_service_input_preserves_runtime_and_wait_contract() {
        let input = json!({
            "baseUrl": "https://www.udio.com",
            "cookieHeader": "a=b",
            "requestBody": { "prompt": "cinematic chorus" },
            "targetAssetKind": "video",
            "waitAudio": false,
            "waitTimeoutMs": 480_000u64,
            "pollIntervalMs": 5_000u64,
            "timeoutMs": 900_000u64,
            "runtimeStateObjectKey": "runtime-123",
            "userAgent": "udio-agent",
            "acceptLanguage": "en-US",
            "origin": "https://www.udio.com",
            "referer": "https://www.udio.com/create"
        });

        let prepared =
            prepare_udio_browser_executor_service_input(&input).expect("udio service input");

        assert_eq!(prepared.base_url, "https://www.udio.com");
        assert_eq!(prepared.request_body["prompt"], "cinematic chorus");
        assert!(matches!(
            prepared.target_asset_kind,
            udio::UdioOutputKind::Video
        ));
        assert!(!prepared.wait_audio);
        assert_eq!(prepared.wait_timeout, std::time::Duration::from_secs(480));
        assert_eq!(prepared.poll_interval, std::time::Duration::from_secs(5));
        assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
        assert_eq!(
            prepared.runtime_state_object_key.as_deref(),
            Some("runtime-123")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "cookie")
                .as_deref(),
            Some("a=b")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "user-agent")
                .as_deref(),
            Some("udio-agent")
        );
    }

    #[test]
    fn prepare_udio_browser_executor_service_input_requires_request_body_contract() {
        let input = json!({
            "baseUrl": "https://www.udio.com"
        });

        let error = prepare_udio_browser_executor_service_input(&input)
            .expect_err("missing request body should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_missing_request_body")
        );
        assert_eq!(error.http_status, Some(400));
    }
}
