use serde_json::{json, Value};
use time::OffsetDateTime;

use super::{
    GeminiAuthSecretEdit, GeminiGeneratedCredentialDraft, GeminiWebCaptureOutput,
    GEMINI_CANVAS_CHAT_API_BASE_URL,
};

pub(super) fn build_canvas_generated_drafts(
    runtime_state_object_key: &str,
    suggested_share_id: &str,
    browser_runtime_state_object_key: Option<&str>,
    captured_api_keys: Option<&[String]>,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    let normalized_api_keys = captured_api_keys
        .unwrap_or(&[])
        .iter()
        .map(|candidate| candidate.trim())
        .filter(|candidate| !candidate.is_empty())
        .fold(Vec::<String>::new(), |mut keys, candidate| {
            if !keys.iter().any(|existing| existing == candidate) {
                keys.push(candidate.to_string());
            }
            keys
        });
    let mut canvas_extra_body = json!({
        "shareId": suggested_share_id,
    });
    if let Some(browser_runtime_state_object_key) = browser_runtime_state_object_key
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
    {
        canvas_extra_body["browserRuntimeStateObjectKey"] = json!(browser_runtime_state_object_key);
    }
    let mut canvas_chat_extra_body = json!({
        "shareId": suggested_share_id,
        "apiBaseUrl": GEMINI_CANVAS_CHAT_API_BASE_URL,
    });
    if let Some(browser_runtime_state_object_key) = canvas_extra_body
        .get("browserRuntimeStateObjectKey")
        .cloned()
    {
        canvas_chat_extra_body["browserRuntimeStateObjectKey"] = browser_runtime_state_object_key;
    }
    if !normalized_api_keys.is_empty() {
        canvas_chat_extra_body["apiKeys"] = json!(normalized_api_keys);
    }
    vec![
        GeminiGeneratedCredentialDraft {
            provider_id: "gemini-canvas".to_string(),
            credential: json!({
                "id": format!("gemini-canvas-manual-{suffix}"),
                "account_name": format!("Gemini Canvas Manual {suffix}"),
                "runtime_state_object_key": runtime_state_object_key,
                "extra_body": canvas_extra_body,
            }),
            secret_edits: Vec::new(),
        },
        GeminiGeneratedCredentialDraft {
            provider_id: "gemini-canvas-chat".to_string(),
            credential: json!({
                "id": format!("gemini-canvas-chat-manual-{suffix}"),
                "account_name": format!("Gemini Canvas Chat Manual {suffix}"),
                "runtime_state_object_key": runtime_state_object_key,
                "extra_body": canvas_chat_extra_body,
            }),
            secret_edits: Vec::new(),
        },
    ]
}

pub(super) fn build_business_generated_drafts(
    jwt: &str,
    config_id: &str,
    session_name: &str,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    vec![GeminiGeneratedCredentialDraft {
        provider_id: "gemini-business".to_string(),
        credential: json!({
            "id": format!("gemini-business-manual-{suffix}"),
            "account_name": format!("Gemini Business Manual {suffix}"),
            "api_key": "",
            "extra_body": {
                "configId": config_id,
                "session": session_name,
            },
        }),
        secret_edits: vec![GeminiAuthSecretEdit {
            field: "api_key".to_string(),
            operation: "replace".to_string(),
            value: jwt.to_string(),
        }],
    }]
}

pub(super) fn build_web_generated_drafts(
    provider_id: &str,
    api_key: &str,
    auth_token: Option<&str>,
    output: &GeminiWebCaptureOutput,
) -> Vec<GeminiGeneratedCredentialDraft> {
    let suffix = OffsetDateTime::now_utc().unix_timestamp().to_string();
    let mut extra_body = serde_json::Map::new();
    for (key, value) in [
        ("authUser", output.account_index.as_deref()),
        ("accessToken", output.access_token.as_deref()),
        ("buildLabel", output.build_label.as_deref()),
        ("sessionId", output.session_id.as_deref()),
        ("language", output.language.as_deref()),
        ("appPagePath", output.app_page_path.as_deref()),
        ("endpointPath", output.endpoint_path.as_deref()),
        (
            "requestContextHeader",
            output.request_context_header.as_deref(),
        ),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            extra_body.insert(key.to_string(), Value::String(value.to_string()));
        }
    }
    if let Some(model_headers) = output.model_headers.as_ref() {
        for (header_name, extra_body_key) in [
            ("x-goog-ext-525001261-jspb", "modelHeader"),
            ("x-goog-ext-73010989-jspb", "modelHeader2"),
            ("x-goog-ext-73010990-jspb", "modelHeader3"),
        ] {
            if let Some(value) = model_headers
                .get(header_name)
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                extra_body.insert(extra_body_key.to_string(), Value::String(value.to_string()));
            }
        }
    }

    let mut headers = serde_json::Map::new();
    if let Some(referer) = output
        .referer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert("Referer".to_string(), Value::String(referer.to_string()));
    }
    if let Some(account_index) = output
        .account_index
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.insert(
            "X-Goog-AuthUser".to_string(),
            Value::String(account_index.to_string()),
        );
    }

    let mut secret_edits = vec![GeminiAuthSecretEdit {
        field: "api_key".to_string(),
        operation: "replace".to_string(),
        value: api_key.to_string(),
    }];
    if let Some(auth_token) = auth_token.map(str::trim).filter(|value| !value.is_empty()) {
        secret_edits.push(GeminiAuthSecretEdit {
            field: "auth_token".to_string(),
            operation: "replace".to_string(),
            value: auth_token.to_string(),
        });
    }

    vec![GeminiGeneratedCredentialDraft {
        provider_id: provider_id.to_string(),
        credential: json!({
            "id": format!("gemini-web-manual-{suffix}"),
            "account_name": format!("Gemini Web /u/1/ Manual {suffix}"),
            "api_key": "",
            "auth_token": "",
            "headers": headers,
            "extra_body": extra_body,
        }),
        secret_edits,
    }]
}
