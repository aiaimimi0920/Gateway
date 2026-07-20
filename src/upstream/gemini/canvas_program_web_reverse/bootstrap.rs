use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_program_web_reverse as surface;
use crate::protocol::gemini::web_reverse as gemini_web;
use crate::routing::candidate::ProviderAccountPayload;

use super::payload::relay_config_from_payload;

pub const GEMINI_CANVAS_PROGRAM_CREATE_RPCID: &str = "ujx1Bf";

pub fn gemini_canvas_program_create_missing_handle_patch_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas program pure HTTP create completed without emitting a concrete canvas app handle.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_create_missing_handle_patch")
}

pub fn build_program_bootstrap_probe_input(
    payload: &ProviderAccountPayload,
) -> Result<Value, GatewayError> {
    let config = relay_config_from_payload(payload)?;
    let probe = surface::build_program_bootstrap_probe(payload)?;
    Ok(json!({
        "provider": "gemini_canvas_program_bootstrap_probe",
        "input": {
            "runtimeStateObjectKey": config.bootstrap.runtime_state_object_key,
            "shareId": config.bootstrap.share_id,
            "relayWsEndpoint": config.bootstrap.relay_ws_endpoint,
            "clientLabel": config.bootstrap.client_label,
            "canvasProgramHint": config.bootstrap.canvas_program_hint,
            "canvasProgramUrl": config.app_endpoint.canvas_program_url,
            "pageUrl": config.app_endpoint.page_url,
            "appPath": config.app_endpoint.app_path,
            "conversationId": config.app_endpoint.conversation_id,
            "responseId": config.app_endpoint.response_id,
            "probe": probe,
        }
    }))
}

pub fn build_program_create_pure_http_request(
    share_id: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    reqid: u64,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let share_id = share_id.trim();
    if share_id.is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Canvas program pure HTTP create requires a shareId.",
        )
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_create_missing_share_id"));
    }

    let inner_arg =
        serde_json::to_string(&json!([Value::Null, share_id, [4]])).map_err(|error| {
            GatewayError::server_error(format!(
                "serialize Gemini Canvas pure HTTP create inner payload: {error}"
            ))
            .with_provider("gemini_canvas_program_web_reverse_compatible")
            .with_code("gemini_canvas_program_create_serialize_failed")
        })?;
    let f_req = serde_json::to_string(&json!([[[
        GEMINI_CANVAS_PROGRAM_CREATE_RPCID,
        inner_arg,
        Value::Null,
        "generic"
    ]]]))
    .map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas pure HTTP create outer payload: {error}"
        ))
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_create_serialize_failed")
    })?;

    let mut query = vec![
        (
            "rpcids".to_string(),
            GEMINI_CANVAS_PROGRAM_CREATE_RPCID.to_string(),
        ),
        ("source-path".to_string(), format!("/share/{share_id}")),
        ("hl".to_string(), bootstrap.language.clone()),
        ("_reqid".to_string(), reqid.to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if let Some(access_token) = bootstrap
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        form.push(("at".to_string(), access_token.to_string()));
    }

    Ok(gemini_web::GeminiWebRequest { query, form })
}

pub fn current_program_create_reqid() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    360_000 + (millis % 100_000)
}

pub fn runtime_patch_from_pure_http_create_response(
    share_id: &str,
    bootstrap_operation: Option<&str>,
    body_text: &str,
    base_url: &str,
) -> Option<HashMap<String, Value>> {
    let app_paths = extract_unique_matches(body_text, r"/app/[0-9a-f]{16}");
    let conversation_ids = extract_unique_matches(body_text, r"\bc_[0-9a-f]{16}\b");
    let response_ids = extract_unique_matches(body_text, r"\br_[0-9a-f]{16}\b");
    let has_canvas_proxy_client = body_text.contains("Browser API Proxy Client");

    let matched_suffix = conversation_ids
        .iter()
        .map(|value| value.trim_start_matches("c_"))
        .find(|suffix| {
            app_paths
                .iter()
                .any(|path| path == &format!("/app/{suffix}"))
        })
        .map(str::to_string);

    let mut app_path = matched_suffix
        .as_deref()
        .map(|suffix| format!("/app/{suffix}"))
        .or_else(|| app_paths.first().cloned());
    let conversation_id = matched_suffix
        .as_deref()
        .map(|suffix| format!("c_{suffix}"))
        .or_else(|| conversation_ids.first().cloned());
    if app_path.is_none() {
        if let Some(conversation_id) = conversation_id.as_deref() {
            app_path = Some(format!("/app/{}", conversation_id.trim_start_matches("c_")));
        }
    }
    let response_id = response_ids.first().cloned();
    let canvas_program_url = app_path
        .as_deref()
        .map(|path| format!("{}{path}", base_url.trim_end_matches('/')));
    let app_path = app_path?;
    let conversation_id = conversation_id?;
    let response_id = response_id?;
    let canvas_program_url = canvas_program_url?;

    let mut patch = HashMap::new();
    patch.insert(
        "shareId".to_string(),
        Value::String(share_id.trim().to_string()),
    );
    patch.insert(
        "shareFollowKind".to_string(),
        Value::String("pure_http_create".to_string()),
    );
    if let Some(operation) = bootstrap_operation
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        patch.insert(
            "canvasProgramOperation".to_string(),
            Value::String(operation.to_string()),
        );
        patch.insert(
            "bootstrapOperation".to_string(),
            Value::String(operation.to_string()),
        );
    }
    patch.insert(
        "canvasProgramUrl".to_string(),
        Value::String(canvas_program_url.clone()),
    );
    patch.insert(
        "pageUrl".to_string(),
        Value::String(canvas_program_url.clone()),
    );
    patch.insert("appPath".to_string(), Value::String(app_path.clone()));
    patch.insert(
        "programId".to_string(),
        Value::String(app_path.trim_start_matches("/app/").to_string()),
    );
    patch.insert(
        "conversationId".to_string(),
        Value::String(conversation_id.clone()),
    );
    patch.insert("responseId".to_string(), Value::String(response_id.clone()));
    patch.insert(
        "lastSeenConversationId".to_string(),
        Value::String(conversation_id.clone()),
    );
    patch.insert(
        "lastSeenResponseId".to_string(),
        Value::String(response_id.clone()),
    );

    let pair = json!({
        "appPath": app_path,
        "programUrl": canvas_program_url,
        "conversationId": conversation_id,
        "responseId": response_id,
        "sourceSurface": if has_canvas_proxy_client { Value::String("canvas_proxy_client".to_string()) } else { Value::Null },
        "sourceRpc": GEMINI_CANVAS_PROGRAM_CREATE_RPCID,
    });
    patch.insert("stableProgramPair".to_string(), pair.clone());
    patch.insert("latestResponsePair".to_string(), pair.clone());
    patch.insert("candidatePairs".to_string(), Value::Array(vec![pair]));
    patch.insert(
        "aggregateHints".to_string(),
        json!({
            "appPaths": app_paths,
            "conversationIds": conversation_ids,
            "responseIds": response_ids,
        }),
    );

    Some(patch)
}

pub fn build_program_bootstrap_invocation_input(
    payload: &ProviderAccountPayload,
    bootstrap_operation: &str,
    bootstrap_prompt: &str,
    locale: &str,
    timeout: std::time::Duration,
) -> Result<Value, GatewayError> {
    let config = relay_config_from_payload(payload)?;
    Ok(build_program_bootstrap_invocation_input_from_config(
        payload.base_url.trim_end_matches('/'),
        &config,
        bootstrap_operation,
        bootstrap_prompt,
        locale,
        timeout,
    ))
}

pub fn build_program_bootstrap_invocation_input_from_config(
    base_url: &str,
    config: &surface::GeminiCanvasProgramRelayConfig,
    bootstrap_operation: &str,
    bootstrap_prompt: &str,
    locale: &str,
    timeout: std::time::Duration,
) -> Value {
    json!({
        "baseUrl": base_url.trim_end_matches('/'),
        "shareId": config.bootstrap.share_id,
        "runtimeStateObjectKey": config.bootstrap.runtime_state_object_key,
        "operation": "bootstrap_program",
        "discoveryOnly": true,
        "bootstrapOperation": bootstrap_operation,
        "bootstrapPrompt": bootstrap_prompt,
        "locale": locale,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "canvasProgramHint": config.bootstrap.canvas_program_hint,
        "canvasProgramUrl": config.app_endpoint.canvas_program_url,
        "pageUrl": config.app_endpoint.page_url,
        "appPath": config.app_endpoint.app_path,
        "conversationId": config.app_endpoint.conversation_id,
        "responseId": config.app_endpoint.response_id,
        "browserExecutablePath": std::env::var("GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH").ok(),
    })
}

pub fn default_gemini_canvas_program_bootstrap_prompt(operation: &str) -> String {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    match operation {
        "text" => format!("CANVAS_PROGRAM_BOOTSTRAP_TEXT_{marker} Reply with exactly: ok"),
        "music" => format!(
            "CANVAS_PROGRAM_BOOTSTRAP_MUSIC_{marker} A short electronic cue with a clear pulse."
        ),
        "video" => format!(
            "CANVAS_PROGRAM_BOOTSTRAP_VIDEO_{marker} A 3 second clip of a glowing cube rotating on a clean background."
        ),
        _ => format!(
            "CANVAS_PROGRAM_BOOTSTRAP_IMAGE_{marker} A neon badge that says CANVAS PROGRAM, high detail, Requested aspect ratio: 1:1."
        ),
    }
}

fn extract_unique_matches(body_text: &str, pattern: &str) -> Vec<String> {
    let regex = Regex::new(pattern).expect("valid Gemini Canvas program pure HTTP regex");
    let mut values = Vec::new();
    for capture in regex.find_iter(body_text) {
        let value = capture.as_str().trim();
        if !value.is_empty() && !values.iter().any(|existing| existing == value) {
            values.push(value.to_string());
        }
    }
    values
}
