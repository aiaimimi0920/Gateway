use std::collections::HashMap;

use serde_json::Value;

use super::{
    GeminiCanvasProgramAppEndpointContract, GeminiCanvasProgramBootstrapContext,
    GeminiCanvasProgramInvokeContract, GeminiCanvasProgramInvokeTargetCandidate,
};
use crate::error::GatewayError;
use crate::protocol::gemini::canvas_web_reverse;
use crate::routing::candidate::ProviderAccountPayload;

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasProgramRelayConfig {
    pub bootstrap: GeminiCanvasProgramBootstrapContext,
    pub app_endpoint: GeminiCanvasProgramAppEndpointContract,
}

impl GeminiCanvasProgramRelayConfig {
    pub fn has_concrete_handle(&self) -> bool {
        self.app_endpoint.has_concrete_handle()
    }
}

fn read_optional_extra_string(
    extra: Option<&HashMap<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| extra.and_then(|body| body.get(*key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_optional_extra_object<'a>(
    extra: Option<&'a HashMap<String, Value>>,
    keys: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    keys.iter()
        .find_map(|key| extra.and_then(|body| body.get(*key)))
        .and_then(Value::as_object)
}

fn read_optional_object_string(
    object: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.and_then(|body| body.get(*key)))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_optional_object_f64(
    object: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<f64> {
    keys.iter()
        .find_map(|key| object.and_then(|body| body.get(*key)))
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|v| v as f64)))
}

fn read_optional_object_array<'a>(
    object: Option<&'a serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<&'a Vec<Value>> {
    keys.iter()
        .find_map(|key| object.and_then(|body| body.get(*key)))
        .and_then(Value::as_array)
}

pub fn relay_config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasProgramRelayConfig, GatewayError> {
    let runtime = canvas_web_reverse::runtime_from_payload(payload)?;
    let extra = payload.extra_body.as_ref();
    let invoke_contract = read_optional_extra_object(
        extra,
        &[
            "canvasProgramInvokeContract",
            "canvas_program_invoke_contract",
            "programInvokeContract",
            "program_invoke_contract",
        ],
    );

    Ok(GeminiCanvasProgramRelayConfig {
        bootstrap: GeminiCanvasProgramBootstrapContext {
            runtime_state_object_key: runtime.runtime_state_object_key,
            share_id: runtime.share_id,
            api_base_url: runtime.api_base_url,
            relay_ws_endpoint: read_optional_extra_string(
                extra,
                &[
                    "canvasRelayWsEndpoint",
                    "canvas_relay_ws_endpoint",
                    "browserRelayWsEndpoint",
                    "browser_relay_ws_endpoint",
                ],
            ),
            client_label: read_optional_extra_string(
                extra,
                &[
                    "canvasRelayClientLabel",
                    "canvas_relay_client_label",
                    "browserRelayClientLabel",
                    "browser_relay_client_label",
                ],
            ),
            canvas_program_hint: read_optional_extra_string(
                extra,
                &[
                    "canvasProgramHint",
                    "canvas_program_hint",
                    "programHint",
                    "program_hint",
                ],
            ),
        },
        app_endpoint: GeminiCanvasProgramAppEndpointContract {
            canvas_program_url: read_optional_extra_string(
                extra,
                &[
                    "canvasProgramUrl",
                    "canvas_program_url",
                    "programUrl",
                    "program_url",
                ],
            ),
            page_url: read_optional_extra_string(extra, &["pageUrl", "page_url"]),
            app_path: read_optional_extra_string(extra, &["appPath", "app_path"]),
            conversation_id: read_optional_extra_string(
                extra,
                &["conversationId", "conversation_id"],
            ),
            response_id: read_optional_extra_string(extra, &["responseId", "response_id"]),
            invoke_base_url: read_optional_extra_string(
                extra,
                &[
                    "invokeBaseUrl",
                    "invoke_base_url",
                    "appEndpointBaseUrl",
                    "app_endpoint_base_url",
                ],
            ),
            music_ws_url: read_optional_extra_string(
                extra,
                &[
                    "musicWsUrl",
                    "music_ws_url",
                    "appMusicWsUrl",
                    "app_music_ws_url",
                ],
            ),
            video_invoke_path: read_optional_extra_string(
                extra,
                &[
                    "videoInvokePath",
                    "video_invoke_path",
                    "videoPath",
                    "video_path",
                ],
            ),
            canvas_program_action: read_optional_extra_string(
                extra,
                &[
                    "canvasProgramAction",
                    "canvas_program_action",
                    "programAction",
                    "program_action",
                ],
            ),
            canvas_program_action_input: read_optional_extra_string(
                extra,
                &[
                    "canvasProgramActionInput",
                    "canvas_program_action_input",
                    "programActionInput",
                    "program_action_input",
                ],
            ),
            canvas_program_invoke_contract: invoke_contract.map(|object| {
                GeminiCanvasProgramInvokeContract {
                    operation: read_optional_object_string(
                        Some(object),
                        &["operation", "bootstrapOperation", "bootstrap_operation"],
                    ),
                    transport_kind: read_optional_object_string(
                        Some(object),
                        &["transportKind", "transport_kind"],
                    ),
                    target: read_optional_object_string(Some(object), &["target", "url", "path"]),
                    target_source: read_optional_object_string(
                        Some(object),
                        &["targetSource", "target_source"],
                    ),
                    target_mime_type: read_optional_object_string(
                        Some(object),
                        &[
                            "targetMimeType",
                            "target_mime_type",
                            "mimeType",
                            "mime_type",
                        ],
                    ),
                    target_candidates: read_optional_object_array(
                        Some(object),
                        &["targetCandidates", "target_candidates"],
                    )
                    .map(|entries| {
                        entries
                            .iter()
                            .filter_map(Value::as_object)
                            .map(|entry| GeminiCanvasProgramInvokeTargetCandidate {
                                url: read_optional_object_string(Some(entry), &["url"]),
                                source: read_optional_object_string(Some(entry), &["source"]),
                                mime_type: read_optional_object_string(
                                    Some(entry),
                                    &["mimeType", "mime_type"],
                                ),
                                kind: read_optional_object_string(Some(entry), &["kind"]),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                    ws_url: read_optional_object_string(
                        Some(object),
                        &["wsUrl", "ws_url", "websocketUrl", "websocket_url"],
                    ),
                    api_style: read_optional_object_string(
                        Some(object),
                        &["apiStyle", "api_style"],
                    ),
                    request_path: read_optional_object_string(
                        Some(object),
                        &["requestPath", "request_path", "path"],
                    ),
                    request_envelope_kind: read_optional_object_string(
                        Some(object),
                        &["requestEnvelopeKind", "request_envelope_kind"],
                    ),
                    request_url: read_optional_object_string(
                        Some(object),
                        &["requestUrl", "request_url"],
                    ),
                    request_body: read_optional_object_string(
                        Some(object),
                        &["requestBody", "request_body"],
                    ),
                    request_rpc_id: read_optional_object_string(
                        Some(object),
                        &["requestRpcId", "request_rpc_id"],
                    ),
                    response_rpc_id: read_optional_object_string(
                        Some(object),
                        &["responseRpcId", "response_rpc_id"],
                    ),
                    source_path: read_optional_object_string(
                        Some(object),
                        &["sourcePath", "source_path"],
                    ),
                    model_hint: read_optional_object_string(
                        Some(object),
                        &["modelHint", "model_hint"],
                    ),
                    cookie_header: read_optional_object_string(
                        Some(object),
                        &["cookieHeader", "cookie_header"],
                    ),
                    action_name: read_optional_object_string(
                        Some(object),
                        &["actionName", "action_name"],
                    ),
                    action_input: read_optional_object_string(
                        Some(object),
                        &["actionInput", "action_input"],
                    ),
                    prompt: read_optional_object_string(Some(object), &["prompt"]),
                    duration_seconds: read_optional_object_f64(
                        Some(object),
                        &["durationSeconds", "duration_seconds", "duration"],
                    ),
                    aspect_ratio: read_optional_object_string(
                        Some(object),
                        &["aspectRatio", "aspect_ratio"],
                    ),
                    ui_state: read_optional_object_string(Some(object), &["uiState", "ui_state"]),
                }
            }),
        },
    })
}
