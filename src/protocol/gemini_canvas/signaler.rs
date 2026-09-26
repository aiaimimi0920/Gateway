use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

use crate::error::GatewayError;

pub fn extract_signaler_account_id_from_page_blob(blob: &str) -> Option<String> {
    static SIGNALER_ACCOUNT_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_ACCOUNT_ID_REGEX
        .get_or_init(|| {
            Regex::new(r#""S06Grb":"([^"]+)""#)
                .expect("gemini canvas signaler account id regex must compile")
        })
        .captures(blob)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn extract_signaler_account_id_from_runtime_state(storage_state: &Value) -> Option<String> {
    let extract_from_value = |value: &Value| -> Option<String> {
        for key in ["signalerAccountId", "googleAccountId", "accountId"] {
            if let Some(found) = value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
            {
                return Some(found.to_string());
            }
        }
        None
    };

    if let Some(found) = extract_from_value(storage_state) {
        return Some(found);
    }

    for wrapper_key in [
        "storageState",
        "runtimeState",
        "browserState",
        "metadata",
        "state",
    ] {
        if let Some(found) = storage_state.get(wrapper_key).and_then(extract_from_value) {
            return Some(found);
        }
    }

    None
}

pub fn build_image_edit_signaler_choose_server_body(account_id: &str) -> String {
    json!([[
        null,
        null,
        null,
        [9, 5],
        null,
        [["assistant-bard"], [1], [[["async_bard"], [account_id]]]],
        null,
        null,
        0,
        0
    ]])
    .to_string()
}

pub fn build_image_edit_signaler_open_channel_body(account_id: &str) -> String {
    let assistant_entry = |channel: &str, include_account: bool| {
        let channel_payload = if include_account {
            json!([[[channel], [account_id]]])
        } else {
            json!([[[channel]]])
        };
        json!([
            null,
            null,
            null,
            [9, 5],
            null,
            [["assistant-bard"], [1], channel_payload],
            null,
            null,
            1
        ])
    };
    let sync_entry = json!([
        null,
        null,
        null,
        [9, 5],
        null,
        [["assistant-bard"], [null, 1], [[["bard-client-sync"]]]],
        null,
        null,
        1
    ]);
    let request_entries = vec![
        (
            "req0___data__",
            json!([[[1, assistant_entry("async_bard", true), null, 3]]]),
        ),
        (
            "req1___data__",
            json!([[[2, assistant_entry("beyond_agency_bard", true), null, 3]]]),
        ),
        (
            "req2___data__",
            json!([[[
                3,
                assistant_entry("mini_app_generation_bard", true),
                null,
                3
            ]]]),
        ),
        ("req3___data__", json!([[[4, sync_entry, null, 3]]])),
    ];
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("count", "4");
    serializer.append_pair("ofs", "0");
    for (key, value) in request_entries {
        serializer.append_pair(&key, &value.to_string());
    }
    serializer.finish()
}

pub fn build_image_edit_signaler_refresh_creds_body(refresh_token: &str) -> String {
    json!([refresh_token]).to_string()
}

pub fn parse_signaler_choose_server_response(body: &str) -> Result<String, GatewayError> {
    let parsed: Vec<Value> = serde_json::from_str(body.trim()).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas signaler chooseServer response: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_signaler_choose_server_invalid")
    })?;
    parsed
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas signaler chooseServer response did not expose a gsessionid."
                    .to_string(),
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_signaler_choose_server_missing_gsessionid")
        })
}

pub fn parse_signaler_open_channel_sid(body: &str) -> Result<String, GatewayError> {
    static SIGNALER_SID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_SID_REGEX
        .get_or_init(|| {
            Regex::new(r#"\["c","([^"]+)""#).expect("gemini canvas signaler sid regex must compile")
        })
        .captures(body)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas signaler open-channel response did not expose a SID.".to_string(),
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_signaler_open_missing_sid")
        })
}

pub fn extract_signaler_long_poll_refresh_token(body: &str) -> Option<String> {
    static SIGNALER_REFRESH_TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_REFRESH_TOKEN_REGEX
        .get_or_init(|| {
            Regex::new(r#"null,null,\["([^"]+)"\]"#)
                .expect("gemini canvas signaler refresh token regex must compile")
        })
        .captures_iter(body)
        .filter_map(|captures| captures.get(1).map(|capture| capture.as_str().trim()))
        .find(|value| !value.is_empty() && value.chars().any(|ch| !ch.is_ascii_digit()))
        .map(ToString::to_string)
}

pub fn extract_signaler_long_poll_max_aid(body: &str) -> Option<u64> {
    static SIGNALER_AID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_AID_REGEX
        .get_or_init(|| {
            Regex::new(r#"\[\[(\d+),"#).expect("gemini canvas signaler aid regex must compile")
        })
        .captures_iter(body)
        .filter_map(|captures| {
            captures
                .get(1)
                .and_then(|value| value.as_str().parse::<u64>().ok())
        })
        .max()
}

pub fn extract_signaler_app_paths(body: &str) -> Vec<String> {
    static SIGNALER_APP_PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    static SIGNALER_DECIMAL_APP_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = SIGNALER_APP_PATH_REGEX.get_or_init(|| {
        Regex::new(r#"(?:/app/|c_)([0-9a-f]{8,})"#)
            .expect("gemini canvas signaler app path regex must compile")
    });
    let decimal_regex = SIGNALER_DECIMAL_APP_ID_REGEX.get_or_init(|| {
        Regex::new(r#"\["(\d{13,})"\]"#)
            .expect("gemini canvas signaler decimal app id regex must compile")
    });
    let mut paths = Vec::new();
    for capture in regex.captures_iter(body) {
        let Some(id) = capture.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let path = format!("/app/{id}");
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
    for capture in decimal_regex.captures_iter(body) {
        let Some(id) = capture.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let path = format!("/app/{id}");
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
    paths
}
