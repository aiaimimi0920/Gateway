use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_executor_helpers::{
    missing_browser_executor_field_error, read_json_bool, read_json_u64,
};
use crate::upstream::browser_worker_runtime_helpers::lumalabs_browser_worker_script_path;
use crate::upstream::browser_worker_types::LumalabsBrowserWorkerInput;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedLumalabsBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) realm_id: String,
    pub(crate) media_operation: Option<String>,
    pub(crate) artifact_field: String,
    pub(crate) session_token: String,
    pub(crate) action_body: Value,
    pub(crate) auto_discover_action_type: bool,
    pub(crate) locale: String,
    pub(crate) timeout: Duration,
}

#[derive(Debug)]
pub(crate) struct PreparedLumalabsBrowserWorkerLaunch {
    pub(crate) node_bin: String,
    pub(crate) script_path: PathBuf,
    pub(crate) stdin_json: Vec<u8>,
}

pub(crate) fn lumalabs_locale(payload: &ProviderAccountPayload) -> &str {
    payload
        .headers
        .get("accept-language")
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("zh-CN")
}

pub(crate) fn build_lumalabs_browser_worker_input<'a>(
    base_url: &'a str,
    realm_id: &'a str,
    media_operation: Option<&'a str>,
    artifact_field: &'a str,
    session_token: &'a str,
    action_body: &'a Value,
    auto_discover_action_type: bool,
    locale: &'a str,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> LumalabsBrowserWorkerInput<'a> {
    LumalabsBrowserWorkerInput {
        base_url,
        realm_id,
        media_operation,
        artifact_field,
        session_token,
        action_body,
        auto_discover_action_type,
        locale,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        browser_executable_path,
    }
}

pub(crate) fn build_lumalabs_browser_worker_input_from_prepared<'a>(
    prepared: &'a PreparedLumalabsBrowserExecutorServiceInput,
    browser_executable_path: Option<String>,
) -> LumalabsBrowserWorkerInput<'a> {
    build_lumalabs_browser_worker_input(
        &prepared.base_url,
        &prepared.realm_id,
        prepared.media_operation.as_deref(),
        &prepared.artifact_field,
        &prepared.session_token,
        &prepared.action_body,
        prepared.auto_discover_action_type,
        &prepared.locale,
        prepared.timeout,
        browser_executable_path,
    )
}

pub(crate) fn serialize_lumalabs_browser_worker_input_from_prepared(
    prepared: &PreparedLumalabsBrowserExecutorServiceInput,
    browser_executable_path: Option<String>,
) -> Result<Vec<u8>, crate::error::GatewayError> {
    let input =
        build_lumalabs_browser_worker_input_from_prepared(prepared, browser_executable_path);
    serde_json::to_vec(&input).map_err(|error| {
        crate::protocol::lumalabs::browser_worker_input_serialize_error(error.to_string().as_str())
    })
}

pub(crate) fn prepare_lumalabs_browser_worker_launch(
    prepared: &PreparedLumalabsBrowserExecutorServiceInput,
    browser_executable_path: Option<String>,
    node_bin: Option<String>,
) -> Result<PreparedLumalabsBrowserWorkerLaunch, crate::error::GatewayError> {
    Ok(PreparedLumalabsBrowserWorkerLaunch {
        node_bin: node_bin.unwrap_or_else(|| "node".to_string()),
        script_path: lumalabs_browser_worker_script_path(),
        stdin_json: serialize_lumalabs_browser_worker_input_from_prepared(
            prepared,
            browser_executable_path,
        )?,
    })
}

pub(crate) fn build_lumalabs_browser_executor_payload(
    base_url: &str,
    realm_id: &str,
    media_operation: Option<&str>,
    artifact_field: &str,
    session_token: &str,
    action_body: &Value,
    auto_discover_action_type: bool,
    locale: &str,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> serde_json::Value {
    serde_json::json!({
        "baseUrl": base_url,
        "realmId": realm_id,
        "mediaOperation": media_operation,
        "artifactField": artifact_field,
        "sessionToken": session_token,
        "actionBody": action_body,
        "autoDiscoverActionType": auto_discover_action_type,
        "locale": locale,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "browserExecutablePath": browser_executable_path,
    })
}

pub(crate) fn build_lumalabs_browser_executor_payload_from_prepared(
    prepared: &PreparedLumalabsBrowserExecutorServiceInput,
    browser_executable_path: Option<String>,
) -> serde_json::Value {
    build_lumalabs_browser_executor_payload(
        &prepared.base_url,
        &prepared.realm_id,
        prepared.media_operation.as_deref(),
        &prepared.artifact_field,
        &prepared.session_token,
        &prepared.action_body,
        prepared.auto_discover_action_type,
        &prepared.locale,
        prepared.timeout,
        browser_executable_path,
    )
}

pub(crate) fn prepare_lumalabs_browser_executor_service_input(
    input: &Value,
) -> Result<PreparedLumalabsBrowserExecutorServiceInput, crate::error::GatewayError> {
    let base_url = input
        .get("baseUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "LumaLabs",
                "baseUrl",
                "browser_executor_missing_base_url",
            )
        })?;
    let realm_id = input
        .get("realmId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "LumaLabs",
                "realmId",
                "browser_executor_missing_realm_id",
            )
        })?;
    let session_token = input
        .get("sessionToken")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "LumaLabs",
                "sessionToken",
                "browser_executor_missing_session_token",
            )
        })?;
    let action_body = input.get("actionBody").cloned().ok_or_else(|| {
        missing_browser_executor_field_error(
            "LumaLabs",
            "actionBody",
            "browser_executor_missing_action_body",
        )
    })?;

    Ok(PreparedLumalabsBrowserExecutorServiceInput {
        base_url,
        realm_id,
        media_operation: input
            .get("mediaOperation")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        artifact_field: input
            .get("artifactField")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("image")
            .to_string(),
        session_token,
        action_body,
        auto_discover_action_type: read_json_bool(input, "autoDiscoverActionType").unwrap_or(false),
        locale: input
            .get("locale")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("en-US")
            .to_string(),
        timeout: Duration::from_millis(read_json_u64(input, "timeoutMs").unwrap_or(180_000)),
    })
}

#[cfg(test)]
mod tests {
    use crate::protocol::lumalabs;
    use serde_json::json;

    use super::*;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        serde_json::from_value(json!({
            "adapter": adapter, "base_url": base_url, "api_key": "sk-test"
        }))
        .expect("default account payload")
    }

    #[test]
    fn lumalabs_locale_prefers_trimmed_accept_language_and_defaults_to_zh_cn() {
        let mut payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
        payload.headers.insert(
            "accept-language".to_string(),
            "  en-US ,en;q=0.9  ".to_string(),
        );
        assert_eq!(lumalabs_locale(&payload), "en-US");

        payload
            .headers
            .insert("accept-language".to_string(), "   ".to_string());
        assert_eq!(lumalabs_locale(&payload), "zh-CN");
    }

    #[test]
    fn build_lumalabs_browser_worker_input_preserves_runtime_contract() {
        let action_body = json!({ "prompt": "volumetric sky whale" });

        let input = build_lumalabs_browser_worker_input(
            "https://app.lumalabs.ai",
            "realm-123",
            Some("create_image_uni_1"),
            "image",
            "session-token",
            &action_body,
            true,
            "en-US",
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );
        let payload = serde_json::to_value(&input).expect("lumalabs worker input");

        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "create_image_uni_1");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn build_lumalabs_browser_worker_input_from_prepared_preserves_runtime_contract() {
        let prepared = PreparedLumalabsBrowserExecutorServiceInput {
            base_url: "https://app.lumalabs.ai".to_string(),
            realm_id: "realm-123".to_string(),
            media_operation: Some("create_image_uni_1".to_string()),
            artifact_field: "image".to_string(),
            session_token: "session-token".to_string(),
            action_body: json!({ "prompt": "volumetric sky whale" }),
            auto_discover_action_type: true,
            locale: "en-US".to_string(),
            timeout: std::time::Duration::from_secs(900),
        };

        let input = build_lumalabs_browser_worker_input_from_prepared(
            &prepared,
            Some("C:/browser/chrome.exe".to_string()),
        );
        let payload = serde_json::to_value(&input).expect("lumalabs worker input");

        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "create_image_uni_1");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn serialize_lumalabs_browser_worker_input_from_prepared_preserves_runtime_contract() {
        let prepared = PreparedLumalabsBrowserExecutorServiceInput {
            base_url: "https://app.lumalabs.ai".to_string(),
            realm_id: "realm-123".to_string(),
            media_operation: Some("create_image_uni_1".to_string()),
            artifact_field: "image".to_string(),
            session_token: "session-token".to_string(),
            action_body: json!({ "prompt": "volumetric sky whale" }),
            auto_discover_action_type: true,
            locale: "en-US".to_string(),
            timeout: std::time::Duration::from_secs(900),
        };

        let stdin_json = serialize_lumalabs_browser_worker_input_from_prepared(
            &prepared,
            Some("C:/browser/chrome.exe".to_string()),
        )
        .expect("lumalabs worker stdin");
        let payload: serde_json::Value =
            serde_json::from_slice(&stdin_json).expect("stdin json should parse");

        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "create_image_uni_1");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn prepare_lumalabs_browser_worker_launch_preserves_runtime_contract() {
        let prepared = PreparedLumalabsBrowserExecutorServiceInput {
            base_url: "https://app.lumalabs.ai".to_string(),
            realm_id: "realm-123".to_string(),
            media_operation: Some("create_image_uni_1".to_string()),
            artifact_field: "image".to_string(),
            session_token: "session-token".to_string(),
            action_body: json!({ "prompt": "volumetric sky whale" }),
            auto_discover_action_type: true,
            locale: "en-US".to_string(),
            timeout: std::time::Duration::from_secs(900),
        };

        let launch = prepare_lumalabs_browser_worker_launch(
            &prepared,
            Some("C:/browser/chrome.exe".to_string()),
            Some("node-custom".to_string()),
        )
        .expect("lumalabs worker launch");
        let payload: serde_json::Value =
            serde_json::from_slice(&launch.stdin_json).expect("stdin json should parse");

        assert_eq!(launch.node_bin, "node-custom");
        assert_eq!(
            launch
                .script_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("lumalabs-browser-worker.mjs")
        );
        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "create_image_uni_1");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn build_lumalabs_browser_executor_payload_preserves_runtime_contract() {
        let action_body = json!({ "prompt": "volumetric sky whale" });

        let payload = build_lumalabs_browser_executor_payload(
            "https://app.lumalabs.ai",
            "realm-123",
            Some(lumalabs::media_operation_name(
                lumalabs::LumalabsMediaOperation::Image,
            )),
            "image",
            "session-token",
            &action_body,
            true,
            "en-US",
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "image");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn build_lumalabs_browser_executor_payload_from_prepared_preserves_runtime_contract() {
        let prepared = PreparedLumalabsBrowserExecutorServiceInput {
            base_url: "https://app.lumalabs.ai".to_string(),
            realm_id: "realm-123".to_string(),
            media_operation: Some("image".to_string()),
            artifact_field: "image".to_string(),
            session_token: "session-token".to_string(),
            action_body: json!({ "prompt": "volumetric sky whale" }),
            auto_discover_action_type: true,
            locale: "en-US".to_string(),
            timeout: std::time::Duration::from_secs(900),
        };

        let payload = build_lumalabs_browser_executor_payload_from_prepared(
            &prepared,
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://app.lumalabs.ai");
        assert_eq!(payload["realmId"], "realm-123");
        assert_eq!(payload["mediaOperation"], "image");
        assert_eq!(payload["artifactField"], "image");
        assert_eq!(payload["sessionToken"], "session-token");
        assert_eq!(payload["actionBody"]["prompt"], "volumetric sky whale");
        assert_eq!(payload["autoDiscoverActionType"], true);
        assert_eq!(payload["locale"], "en-US");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    }

    #[test]
    fn prepare_lumalabs_browser_executor_service_input_preserves_runtime_contract() {
        let input = json!({
            "baseUrl": "https://app.lumalabs.ai",
            "realmId": "realm-123",
            "mediaOperation": "image",
            "artifactField": "image",
            "sessionToken": "session-token",
            "actionBody": { "prompt": "volumetric sky whale" },
            "autoDiscoverActionType": true,
            "locale": "en-US",
            "timeoutMs": 900_000u64
        });

        let prepared = prepare_lumalabs_browser_executor_service_input(&input)
            .expect("lumalabs service input");

        assert_eq!(prepared.base_url, "https://app.lumalabs.ai");
        assert_eq!(prepared.realm_id, "realm-123");
        assert_eq!(prepared.media_operation.as_deref(), Some("image"));
        assert_eq!(prepared.artifact_field, "image");
        assert_eq!(prepared.session_token, "session-token");
        assert_eq!(prepared.action_body["prompt"], "volumetric sky whale");
        assert!(prepared.auto_discover_action_type);
        assert_eq!(prepared.locale, "en-US");
        assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
    }

    #[test]
    fn prepare_lumalabs_browser_executor_service_input_requires_action_body_contract() {
        let input = json!({
            "baseUrl": "https://app.lumalabs.ai",
            "realmId": "realm-123",
            "sessionToken": "session-token"
        });

        let error = prepare_lumalabs_browser_executor_service_input(&input)
            .expect_err("missing action body should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_missing_action_body")
        );
        assert_eq!(error.http_status, Some(400));
    }
}
