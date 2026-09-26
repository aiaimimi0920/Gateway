//! Normalize Canvas runtime handles and program provenance.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::{
    canonicalize_folder_sync_raw_source, read_optional_object_string,
    read_optional_object_string_array,
};
use crate::db;
use crate::error::GatewayError;
use crate::protocol::gemini::shared::GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;
use serde_json::Value;

pub(super) fn normalize_gemini_canvas_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini canvas provider credential payload 必须是 JSON object",
        ));
    };

    let runtime_state_object_key = read_optional_object_string(
        raw_map,
        &["runtimeStateObjectKey", "runtime_state_object_key"],
    )
    .ok_or_else(|| {
        GatewayError::bad_request(
            "gemini canvas provider credential payload 缺少 runtimeStateObjectKey。",
        )
    })?;
    let share_url =
        read_optional_object_string(raw_map, &["shareUrl", "share_url"]).or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareUrl", "share_url"]))
        });
    let share_id = read_optional_object_string(raw_map, &["shareId", "share_id"])
        .or_else(|| {
            read_optional_object_string(raw_map, &["suggestedShareId", "suggested_share_id"])
        })
        .or_else(|| {
            share_url
                .as_deref()
                .and_then(crate::protocol::gemini_canvas::share_id_from_share_url)
        })
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareId", "share_id"]))
        })
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["shareUrl", "share_url"]))
                .and_then(|value| crate::protocol::gemini_canvas::share_id_from_share_url(&value))
        });
    let api_base_url = read_optional_object_string(raw_map, &["apiBaseUrl", "api_base_url"])
        .or_else(|| {
            provider_account
                .payload
                .get("extraBody")
                .or_else(|| provider_account.payload.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| {
                    read_optional_object_string(extra, &["apiBaseUrl", "api_base_url"])
                })
        });
    let canvas_program_url = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramUrl",
            "canvas_program_url",
            "programUrl",
            "program_url",
        ],
    )
    .or_else(|| {
        provider_account
            .payload
            .get("extraBody")
            .or_else(|| provider_account.payload.get("extra_body"))
            .and_then(Value::as_object)
            .and_then(|extra| {
                read_optional_object_string(
                    extra,
                    &[
                        "canvasProgramUrl",
                        "canvas_program_url",
                        "programUrl",
                        "program_url",
                    ],
                )
            })
    });
    let canvas_program_hint = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramHint",
            "canvas_program_hint",
            "programHint",
            "program_hint",
        ],
    )
    .or_else(|| {
        provider_account
            .payload
            .get("extraBody")
            .or_else(|| provider_account.payload.get("extra_body"))
            .and_then(Value::as_object)
            .and_then(|extra| {
                read_optional_object_string(
                    extra,
                    &[
                        "canvasProgramHint",
                        "canvas_program_hint",
                        "programHint",
                        "program_hint",
                    ],
                )
            })
    });
    let page_url = read_optional_object_string(raw_map, &["pageUrl", "page_url"]);
    let app_path = read_optional_object_string(raw_map, &["appPath", "app_path"]);
    let conversation_id =
        read_optional_object_string(raw_map, &["conversationId", "conversation_id"]);
    let response_id = read_optional_object_string(raw_map, &["responseId", "response_id"]);
    let invoke_base_url = read_optional_object_string(
        raw_map,
        &[
            "invokeBaseUrl",
            "invoke_base_url",
            "appEndpointBaseUrl",
            "app_endpoint_base_url",
        ],
    );
    let music_ws_url = read_optional_object_string(
        raw_map,
        &[
            "musicWsUrl",
            "music_ws_url",
            "appMusicWsUrl",
            "app_music_ws_url",
        ],
    );
    let video_invoke_path = read_optional_object_string(
        raw_map,
        &[
            "videoInvokePath",
            "video_invoke_path",
            "videoPath",
            "video_path",
        ],
    );
    let canvas_program_action = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramAction",
            "canvas_program_action",
            "programAction",
            "program_action",
        ],
    );
    let canvas_program_action_input = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramActionInput",
            "canvas_program_action_input",
            "programActionInput",
            "program_action_input",
        ],
    );
    let canvas_program_invoke_contract = raw_map
        .get("canvasProgramInvokeContract")
        .or_else(|| raw_map.get("canvas_program_invoke_contract"))
        .or_else(|| raw_map.get("programInvokeContract"))
        .or_else(|| raw_map.get("program_invoke_contract"))
        .filter(|value| value.is_object())
        .cloned();
    let before_url = read_optional_object_string(raw_map, &["beforeUrl", "before_url"]);
    let final_url = read_optional_object_string(raw_map, &["finalUrl", "final_url"]);
    let share_follow_kind =
        read_optional_object_string(raw_map, &["shareFollowKind", "share_follow_kind"]);
    let program_id = read_optional_object_string(raw_map, &["programId", "program_id"]);
    let canvas_program_operation = read_optional_object_string(
        raw_map,
        &[
            "canvasProgramOperation",
            "canvas_program_operation",
            "bootstrapOperation",
            "bootstrap_operation",
        ],
    );
    let last_seen_conversation_id = read_optional_object_string(
        raw_map,
        &["lastSeenConversationId", "last_seen_conversation_id"],
    );
    let last_seen_response_id =
        read_optional_object_string(raw_map, &["lastSeenResponseId", "last_seen_response_id"]);
    let captured_at = read_optional_object_string(raw_map, &["capturedAt", "captured_at"]);
    let last_validated_at =
        read_optional_object_string(raw_map, &["lastValidatedAt", "last_validated_at"]);
    let candidate_pairs = raw_map
        .get("candidatePairs")
        .filter(|value| value.is_array())
        .cloned();
    let aggregate_hints = raw_map
        .get("aggregateHints")
        .filter(|value| value.is_object())
        .cloned();
    let new_chat_clicked = raw_map.get("newChatClicked").and_then(Value::as_bool);
    let mode_selected = raw_map.get("modeSelected").and_then(Value::as_bool);
    let account_name = read_optional_object_string(raw_map, &["accountName", "account_name"])
        .or_else(|| read_optional_object_string(raw_map, &["currentUrl", "current_url"]));
    let credential_material_key = read_optional_object_string(
        raw_map,
        &[
            "credentialMaterialKey",
            "credential_material_key",
            "materialKey",
        ],
    );
    let google_api_key = read_optional_object_string(
        raw_map,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    );
    let api_keys = read_optional_object_string_array(raw_map, &["apiKeys", "api_keys"]);
    let cookie_header = read_optional_object_string(raw_map, &["cookieHeader", "cookie_header"]);
    let program_owned_surface =
        provider_account.adapter == GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;

    let mut payload = serde_json::Map::new();
    payload.insert("apiKey".to_string(), Value::String(String::new()));
    payload.insert(
        "runtimeStateObjectKey".to_string(),
        Value::String(runtime_state_object_key),
    );
    if let Some(account_name) = account_name {
        payload.insert("accountName".to_string(), Value::String(account_name));
    }
    let mut extra_body = serde_json::Map::new();
    if let Some(share_id) = share_id {
        extra_body.insert("shareId".to_string(), Value::String(share_id));
    }
    if let Some(api_base_url) = api_base_url {
        extra_body.insert("apiBaseUrl".to_string(), Value::String(api_base_url));
    }
    if let Some(canvas_program_url) = canvas_program_url {
        extra_body.insert(
            "canvasProgramUrl".to_string(),
            Value::String(canvas_program_url),
        );
    }
    if let Some(canvas_program_hint) = canvas_program_hint {
        extra_body.insert(
            "canvasProgramHint".to_string(),
            Value::String(canvas_program_hint),
        );
    }
    if let Some(page_url) = page_url {
        extra_body.insert("pageUrl".to_string(), Value::String(page_url));
    }
    if let Some(app_path) = app_path {
        extra_body.insert("appPath".to_string(), Value::String(app_path));
    }
    if let Some(conversation_id) = conversation_id {
        extra_body.insert("conversationId".to_string(), Value::String(conversation_id));
    }
    if let Some(response_id) = response_id {
        extra_body.insert("responseId".to_string(), Value::String(response_id));
    }
    if let Some(invoke_base_url) = invoke_base_url {
        extra_body.insert("invokeBaseUrl".to_string(), Value::String(invoke_base_url));
    }
    if let Some(music_ws_url) = music_ws_url {
        extra_body.insert("musicWsUrl".to_string(), Value::String(music_ws_url));
    }
    if let Some(video_invoke_path) = video_invoke_path {
        extra_body.insert(
            "videoInvokePath".to_string(),
            Value::String(video_invoke_path),
        );
    }
    if let Some(canvas_program_action) = canvas_program_action {
        extra_body.insert(
            "canvasProgramAction".to_string(),
            Value::String(canvas_program_action),
        );
    }
    if let Some(canvas_program_action_input) = canvas_program_action_input {
        extra_body.insert(
            "canvasProgramActionInput".to_string(),
            Value::String(canvas_program_action_input),
        );
    }
    if let Some(canvas_program_invoke_contract) = canvas_program_invoke_contract {
        extra_body.insert(
            "canvasProgramInvokeContract".to_string(),
            canvas_program_invoke_contract,
        );
    }
    if let Some(share_url) = share_url {
        extra_body.insert("shareUrl".to_string(), Value::String(share_url));
    }
    if let Some(before_url) = before_url {
        extra_body.insert("beforeUrl".to_string(), Value::String(before_url));
    }
    if let Some(final_url) = final_url {
        extra_body.insert("finalUrl".to_string(), Value::String(final_url));
    }
    if let Some(share_follow_kind) = share_follow_kind {
        extra_body.insert(
            "shareFollowKind".to_string(),
            Value::String(share_follow_kind),
        );
    }
    if let Some(program_id) = program_id {
        extra_body.insert("programId".to_string(), Value::String(program_id));
    }
    if let Some(canvas_program_operation) = canvas_program_operation {
        extra_body.insert(
            "canvasProgramOperation".to_string(),
            Value::String(canvas_program_operation.clone()),
        );
        extra_body.insert(
            "bootstrapOperation".to_string(),
            Value::String(canvas_program_operation),
        );
    }
    if let Some(last_seen_conversation_id) = last_seen_conversation_id {
        extra_body.insert(
            "lastSeenConversationId".to_string(),
            Value::String(last_seen_conversation_id),
        );
    }
    if let Some(last_seen_response_id) = last_seen_response_id {
        extra_body.insert(
            "lastSeenResponseId".to_string(),
            Value::String(last_seen_response_id),
        );
    }
    if let Some(captured_at) = captured_at {
        extra_body.insert("capturedAt".to_string(), Value::String(captured_at));
    }
    if let Some(last_validated_at) = last_validated_at {
        extra_body.insert(
            "lastValidatedAt".to_string(),
            Value::String(last_validated_at),
        );
    }
    if let Some(candidate_pairs) = candidate_pairs {
        extra_body.insert("candidatePairs".to_string(), candidate_pairs);
    }
    if let Some(aggregate_hints) = aggregate_hints {
        extra_body.insert("aggregateHints".to_string(), aggregate_hints);
    }
    if let Some(new_chat_clicked) = new_chat_clicked {
        extra_body.insert("newChatClicked".to_string(), Value::Bool(new_chat_clicked));
    }
    if let Some(mode_selected) = mode_selected {
        extra_body.insert("modeSelected".to_string(), Value::Bool(mode_selected));
    }
    if let Some(cookie_header) = cookie_header {
        extra_body.insert("cookieHeader".to_string(), Value::String(cookie_header));
    }
    if !program_owned_surface {
        if let Some(google_api_key) = google_api_key {
            extra_body.insert("googleApiKey".to_string(), Value::String(google_api_key));
        }
    }
    if !program_owned_surface {
        if let Some(api_keys) = api_keys {
            extra_body.insert(
                "apiKeys".to_string(),
                Value::Array(api_keys.into_iter().map(Value::String).collect()),
            );
        }
    }
    if !extra_body.is_empty() {
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    if let Some(credential_material_key) = credential_material_key {
        payload.insert(
            "credentialMaterialKey".to_string(),
            Value::String(credential_material_key),
        );
    }
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("browser_state")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}
