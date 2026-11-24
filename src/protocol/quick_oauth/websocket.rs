//! Bidirectional WebSocket transport and event state machine for Amazon Quick conversations.

use super::{
    eventstream::{decode_inbound_binary, decode_inbound_text, encode_outbound_event},
    messages::PreparedTurn,
    metadata::RuntimeMetadata,
    APP_VERSION, SURFACE_TYPE,
};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalToolCall, TokenUsage},
};
use futures::SinkExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rustls::{crypto::ring, ClientConfig, RootCertStore};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Error as WsError, Message},
    Connector, MaybeTlsStream, WebSocketStream,
};

pub(crate) type QuickWsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DELETE_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const PING_INTERVAL: Duration = Duration::from_secs(20);
pub(crate) const POST_MESSAGE_GRACE: Duration = Duration::from_secs(5);

pub(crate) struct ActiveTurnSocket {
    pub socket: QuickWsStream,
    pub thread_id: String,
}

#[derive(Default)]
pub(crate) struct TurnAccumulator {
    pub text: String,
    pub saw_text_delta: bool,
    pub usage: Option<TokenUsage>,
    pub native_tools: Vec<CanonicalToolCall>,
    pending_tools: HashMap<String, (Option<String>, String)>,
    pub post_message_grace: bool,
    pub finished: bool,
}

#[derive(Default)]
pub(crate) struct StepDelta {
    pub text_deltas: Vec<String>,
    pub finished: bool,
}

impl TurnAccumulator {
    pub fn apply_ws_message(&mut self, msg: Message) -> Result<StepDelta, GatewayError> {
        let events = match msg {
            Message::Binary(bytes) => decode_inbound_binary(&bytes)?,
            Message::Text(text) => decode_inbound_text(&text)?.into_iter().collect(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                return Ok(StepDelta::default())
            }
            Message::Close(_) => {
                self.finished = true;
                return Ok(StepDelta {
                    text_deltas: Vec::new(),
                    finished: true,
                });
            }
        };
        let mut step = StepDelta::default();
        for ev in events {
            self.apply_event(&ev, &mut step)?;
            if self.finished {
                step.finished = true;
                break;
            }
        }
        Ok(step)
    }

    fn apply_event(&mut self, ev: &Value, step: &mut StepDelta) -> Result<(), GatewayError> {
        let kind = ev.get("type").and_then(Value::as_str).unwrap_or_default();
        let payload = ev.get("payload").unwrap_or(&Value::Null);
        match kind {
            "text_delta" => {
                let piece = payload
                    .as_str()
                    .or_else(|| payload.get("delta").and_then(Value::as_str))
                    .or_else(|| payload.get("text").and_then(Value::as_str))
                    .unwrap_or_default();
                if !piece.is_empty() {
                    self.saw_text_delta = true;
                    self.text.push_str(piece);
                    step.text_deltas.push(piece.to_owned());
                }
            }
            "text_complete" => {
                if !self.saw_text_delta {
                    if let Some(full) = payload
                        .as_str()
                        .or_else(|| payload.get("text").and_then(Value::as_str))
                        .filter(|s| !s.is_empty())
                    {
                        self.text.push_str(full);
                        step.text_deltas.push(full.to_owned());
                    }
                }
            }
            "turn_usage" => {
                let input = u64_field(payload, &["input_tokens", "prompt_tokens"]);
                let output = u64_field(payload, &["output_tokens", "completion_tokens"]);
                let total = payload
                    .get("total_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(input.saturating_add(output));
                let cache_read = opt_u64(payload, "cache_read_tokens");
                let cache_write = opt_u64(payload, "cache_write_tokens");
                let entry = self.usage.get_or_insert_with(TokenUsage::default);
                entry.prompt_tokens = entry.prompt_tokens.saturating_add(input);
                entry.completion_tokens = entry.completion_tokens.saturating_add(output);
                entry.total_tokens = entry.total_tokens.saturating_add(total);
                if let Some(r) = cache_read {
                    entry.cache_read_input_tokens =
                        Some(entry.cache_read_input_tokens.unwrap_or(0).saturating_add(r));
                }
                if let Some(w) = cache_write {
                    entry.cache_creation_input_tokens = Some(
                        entry
                            .cache_creation_input_tokens
                            .unwrap_or(0)
                            .saturating_add(w),
                    );
                }
            }
            "tool_start"
            | "tool_call"
            | "tool_call_update"
            | "tool_arguments"
            | "tool_arguments_edited"
            | "tool_complete" => {
                self.record_tool_event(kind, payload);
            }
            "message_complete" => {
                if let Some(arr) = payload.get("tool_calls").and_then(Value::as_array) {
                    for item in arr {
                        self.record_tool_event("tool_complete", item);
                    }
                }
                if payload.get("continuation_expected") == Some(&Value::Bool(false)) {
                    self.post_message_grace = true;
                }
            }
            "error" | "session_error" | "stream_error" | "turn_failed" => {
                return Err(classify_event_error(kind, payload));
            }
            "turn_completed" | "turn_aborted" | "turn_ended" | "stream_ended" | "chat_complete" => {
                self.flush_pending_tools();
                self.finished = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn record_tool_event(&mut self, kind: &str, payload: &Value) {
        let id = payload
            .get("id")
            .or_else(|| payload.get("tool_call_id"))
            .or_else(|| payload.get("toolUseId"))
            .and_then(Value::as_str)
            .unwrap_or("call_quick")
            .to_owned();
        let name = payload
            .get("name")
            .or_else(|| payload.get("tool_name"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let entry = self
            .pending_tools
            .entry(id.clone())
            .or_insert_with(|| (None, String::new()));
        if let Some(n) = name {
            entry.0 = Some(n);
        }
        if let Some(args) = payload.get("arguments").or_else(|| payload.get("input")) {
            match args {
                Value::String(s) if kind == "tool_arguments" => entry.1.push_str(s),
                Value::String(s) => entry.1 = s.clone(),
                other => {
                    if let Ok(s) = serde_json::to_string(other) {
                        entry.1 = s;
                    }
                }
            }
        }
        if kind == "tool_complete" {
            if let Some((Some(tool_name), args)) = self.pending_tools.remove(&id) {
                self.native_tools.push(CanonicalToolCall {
                    id: Some(id),
                    call_type: "function".to_owned(),
                    name: Some(tool_name),
                    arguments: Some(if args.is_empty() {
                        "{}".to_owned()
                    } else {
                        args
                    }),
                    raw: HashMap::new(),
                });
            }
        }
    }

    pub fn flush_pending_tools(&mut self) {
        for (id, (maybe_name, args)) in std::mem::take(&mut self.pending_tools) {
            if let Some(tool_name) = maybe_name {
                self.native_tools.push(CanonicalToolCall {
                    id: Some(id),
                    call_type: "function".to_owned(),
                    name: Some(tool_name),
                    arguments: Some(if args.is_empty() {
                        "{}".to_owned()
                    } else {
                        args
                    }),
                    raw: HashMap::new(),
                });
            }
        }
    }
}

pub(crate) async fn connect_and_begin_turn(
    meta: &RuntimeMetadata,
    bearer_jwt: &str,
    turn: &PreparedTurn,
    budget: Duration,
) -> Result<ActiveTurnSocket, GatewayError> {
    let url = build_ws_url(&meta.ws_host, bearer_jwt);
    let mut request = url.as_str().into_client_request().map_err(|_| {
        GatewayError::bad_request("Invalid Amazon Quick WebSocket endpoint")
            .with_code("quick_oauth_invalid_ws_url")
    })?;
    let auth_header = HeaderValue::from_str(&format!("Bearer {bearer_jwt}")).map_err(|_| {
        GatewayError::bad_request("Invalid Amazon Quick bearer token")
            .with_code("quick_oauth_invalid_token")
    })?;
    let headers = request.headers_mut();
    headers.insert("Authorization", auth_header);
    headers.insert("Origin", HeaderValue::from_static("file://"));
    headers.insert("X-Surface-Type", HeaderValue::from_static(SURFACE_TYPE));
    headers.insert("X-Outpost-Id", HeaderValue::from_static(""));
    headers.insert(
        "X-Agent-Service-Type",
        HeaderValue::from_static("UNIFIED_QUICK"),
    );

    let connect_cap = budget.min(CONNECT_TIMEOUT);
    let (mut socket, _) = tokio::time::timeout(connect_cap, dial_socket(&url, request))
        .await
        .map_err(|_| {
            GatewayError::service_unavailable("Amazon Quick WebSocket connect timed out")
                .with_code("quick_oauth_connect_timeout")
        })?
        .map_err(classify_ws_connect_error)?;

    let thread_id = uuid::Uuid::new_v4().to_string();
    let trace_id = uuid::Uuid::new_v4().to_string();
    let message_id = uuid::Uuid::new_v4().to_string();
    let mut user_msg = json!({
        "text": turn.prompt,
        "send_origin": "programmatic",
        "had_physical_input": false,
        "surfaceType": SURFACE_TYPE,
        "outpostId": "",
        "appVersion": APP_VERSION,
        "traceId": trace_id,
        "message_id": message_id,
        "thread_id": thread_id,
        "agent_mode": turn.model,
        "user_timezone": "UTC",
        "user_locale": "en-US",
        "chat_agent_id": Value::Null,
        "auto_routing": false,
        "web_search_enabled": false,
        "pre_approved_tools": [],
        "preload_skills": [],
        "panel_edits": [],
        "custom_prompt": Value::Null,
        "system_prompt": turn.system_prompt,
    });
    if let Some(effort) = &turn.thinking_effort {
        user_msg["thinking_effort"] = json!(effort);
    }

    let frames = [
        encode_outbound_event(
            "session_connect",
            json!({
                "actor_id": meta.actor_arn.as_deref().unwrap_or(""),
                "surfaceType": SURFACE_TYPE,
                "outpostId": "",
                "appVersion": APP_VERSION,
                "external_agent_sign_in": 1,
                "recents_promotion": 1,
            }),
        )?,
        encode_outbound_event("federate_status", json!({}))?,
        encode_outbound_event(
            "session_create",
            json!({
                "session_id": thread_id,
                "title": turn.title,
            }),
        )?,
        encode_outbound_event("user_message", user_msg)?,
    ];
    for frame in frames {
        socket
            .send(Message::Binary(frame.into_bytes()))
            .await
            .map_err(|_| {
                GatewayError::service_unavailable("Failed sending Amazon Quick turn frame")
                    .with_code("quick_oauth_ws_send_failed")
            })?;
    }
    Ok(ActiveTurnSocket { socket, thread_id })
}

pub(crate) async fn delete_and_close(mut active: ActiveTurnSocket) {
    let _ = tokio::time::timeout(DELETE_TIMEOUT, async {
        if let Ok(del) =
            encode_outbound_event("session_delete", json!({ "session_id": active.thread_id }))
        {
            let _ = active.socket.send(Message::Binary(del.into_bytes())).await;
        }
        let _ = active.socket.close(None).await;
    })
    .await;
}

fn build_ws_url(ws_host: &str, bearer_jwt: &str) -> String {
    #[cfg(test)]
    if let Ok(override_url) = super::TEST_WS_URL.try_with(|v| v.clone()) {
        return override_url;
    }
    let encoded_jwt = utf8_percent_encode(bearer_jwt, NON_ALPHANUMERIC);
    format!(
        "wss://{ws_host}/streaming/quickwork-chat-bidirectional-stream?surfaceType={SURFACE_TYPE}&token={encoded_jwt}"
    )
}

async fn dial_socket(
    url: &str,
    request: tokio_tungstenite::tungstenite::http::Request<()>,
) -> Result<
    (
        QuickWsStream,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ),
    WsError,
> {
    if url.starts_with("wss://") {
        connect_async_tls_with_config(request, None, false, Some(shared_tls_connector())).await
    } else {
        connect_async(request).await
    }
}

fn shared_tls_connector() -> Connector {
    static CONNECTOR: OnceLock<Connector> = OnceLock::new();
    CONNECTOR
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            let loaded = rustls_native_certs::load_native_certs();
            roots.add_parsable_certificates(loaded.certs);
            let cfg = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("ring TLS versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Connector::Rustls(Arc::new(cfg))
        })
        .clone()
}

fn classify_ws_connect_error(err: WsError) -> GatewayError {
    if let WsError::Http(resp) = err {
        let status = resp.status().as_u16();
        let mut gw = GatewayError::service_unavailable(format!(
            "Amazon Quick WebSocket handshake rejected (HTTP {status})"
        ))
        .with_code("quick_oauth_upstream_rejected");
        gw.http_status = Some(if (300..400).contains(&status) {
            502
        } else {
            status
        });
        return gw;
    }
    GatewayError::service_unavailable("Amazon Quick WebSocket connection failed")
        .with_code("quick_oauth_ws_connect_failed")
}

fn classify_event_error(kind: &str, payload: &Value) -> GatewayError {
    let code = payload
        .get("code")
        .or_else(|| payload.get("error_code"))
        .and_then(Value::as_str)
        .unwrap_or(kind);
    let lower = code.to_ascii_lowercase();
    let mut err =
        GatewayError::service_unavailable(format!("Amazon Quick upstream event error ({code})"))
            .with_code("quick_oauth_upstream_event_error");
    err.http_status = Some(502);
    if lower.contains("throttl") || lower.contains("rate") || lower.contains("quota") {
        err.http_status = Some(429);
    } else if lower.contains("auth") || lower.contains("token") || lower.contains("unauthoriz") {
        err.http_status = Some(401);
    }
    err
}

fn u64_field(payload: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|k| payload.get(*k).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn opt_u64(payload: &Value, key: &str) -> Option<u64> {
    payload.get(key).and_then(Value::as_u64).filter(|v| *v > 0)
}
