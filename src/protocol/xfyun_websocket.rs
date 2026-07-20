use std::collections::{HashMap, VecDeque};
use std::pin::Pin;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::{SinkExt, Stream, StreamExt};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rquest::Method;
use rustls::crypto::ring;
use rustls::ClientConfig;
use rustls::RootCertStore;
use serde_json::{json, Map, Value};
use sha2::Sha256;
use time::format_description::well_known::Rfc2822;
use time::OffsetDateTime;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::Response as WsResponse;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{
    connect_async, connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, warn};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, ContentPart, EndpointKind,
    MessageRole, TokenUsage,
};
use crate::protocol::sse_parse::format_sse_event;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub const XFYUN_WEBSOCKET_DEFAULT_PATH: &str = "/v1.1/chat";

type XfyunWsSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type OpenAiSseByteStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static>>;
type HmacSha256 = Hmac<Sha256>;

struct XfyunWsStreamState {
    socket: XfyunWsSocket,
    pending: VecDeque<Bytes>,
    model: String,
    response_id: String,
    created_at: i64,
    finished: bool,
}

#[derive(Debug, Default)]
struct ParsedXfyunFrame {
    content_fragments: Vec<String>,
    usage: Option<TokenUsage>,
    done: bool,
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    )
}

pub fn pack_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let app_id = read_required_app_id(payload)?;
    let uid = req
        .explicit_session_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| read_payload_string(payload.extra_body.as_ref(), &["uid", "userId"]))
        .unwrap_or_else(|| "gateway".to_string());

    let mut header = Map::new();
    header.insert("app_id".to_string(), Value::String(app_id));
    header.insert("uid".to_string(), Value::String(uid));
    if let Some(patch_id) =
        read_payload_array(payload.extra_body.as_ref(), &["patchId", "patch_id"])
    {
        header.insert("patch_id".to_string(), patch_id);
    }

    let mut chat = Map::new();
    chat.insert("domain".to_string(), Value::String(model.to_string()));

    if let Some(value) = request_value(req, &["temperature"]) {
        chat.insert("temperature".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["top_p"]) {
        chat.insert("top_p".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["top_k"]) {
        chat.insert("top_k".to_string(), value.clone());
    }
    if let Some(value) = request_value(req, &["max_tokens", "max_output_tokens"]) {
        chat.insert("max_tokens".to_string(), value.clone());
    }
    for (key, value) in &req.extra {
        if matches!(
            key.as_str(),
            "temperature" | "top_p" | "top_k" | "max_tokens" | "max_output_tokens"
        ) {
            continue;
        }
        if let Some(normalized) = normalize_parameter_field_name(key) {
            chat.entry(normalized.to_string())
                .or_insert_with(|| value.clone());
        }
    }

    let text_messages = req
        .messages
        .iter()
        .filter_map(pack_message)
        .collect::<Vec<_>>();
    if text_messages.is_empty() {
        return Err(GatewayError::bad_request(
            "XFYun native WebSocket requests require at least one text turn",
        )
        .with_code("xfyun_websocket_missing_messages")
        .with_provider("xfyun_websocket_compatible"));
    }

    Ok(json!({
        "header": Value::Object(header),
        "parameter": {
            "chat": Value::Object(chat),
        },
        "payload": {
            "message": {
                "text": text_messages,
            }
        }
    }))
}

pub fn build_signed_websocket_url(
    payload: &ProviderAccountPayload,
) -> Result<String, GatewayError> {
    let api_secret = read_required_api_secret(payload)?;
    let api_key = payload.api_key.trim();
    if api_key.is_empty() {
        return Err(
            GatewayError::unauthorized("XFYun native WebSocket provider missing APIKey")
                .with_code("xfyun_websocket_missing_api_key")
                .with_provider("xfyun_websocket_compatible"),
        );
    }

    let path = websocket_path(payload);
    let base_url = normalize_websocket_base_url(&payload.base_url)?;
    let (scheme, host, base_path) = split_websocket_base(&base_url)?;
    let full_path = join_paths(&base_path, &path);
    let date = OffsetDateTime::now_utc()
        .format(&Rfc2822)
        .map_err(|error| {
            GatewayError::server_error(format!("failed to format RFC2822 date: {error}"))
                .with_code("xfyun_websocket_date_format_failed")
                .with_provider("xfyun_websocket_compatible")
        })?;
    let signature_origin = format!("host: {host}\ndate: {date}\nGET {full_path} HTTP/1.1");
    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes()).map_err(|error| {
        GatewayError::server_error(format!("failed to initialize HMAC: {error}"))
            .with_code("xfyun_websocket_hmac_init_failed")
            .with_provider("xfyun_websocket_compatible")
    })?;
    mac.update(signature_origin.as_bytes());
    let signature = BASE64_STANDARD.encode(mac.finalize().into_bytes());
    let authorization_origin = format!(
        "api_key=\"{api_key}\", algorithm=\"hmac-sha256\", headers=\"host date request-line\", signature=\"{signature}\""
    );
    let authorization = BASE64_STANDARD.encode(authorization_origin.as_bytes());

    Ok(format!(
        "{scheme}://{host}{full_path}?authorization={authorization_query}&date={date_query}&host={host_query}",
        authorization_query = encode_query_component(&authorization),
        date_query = encode_query_component(&date),
        host_query = encode_query_component(&host),
    ))
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    if !supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "XFYun native WebSocket adapters support only chat/messages/responses style requests",
        )
        .with_code("unsupported_xfyun_websocket_endpoint"));
    }

    Ok(RequestPlan {
        method: Method::POST,
        url: build_signed_websocket_url(payload)?,
        query: Vec::new(),
        body: Some(pack_request(payload, req, model)?),
        response_kind: EndpointKind::ChatCompletions,
    })
}

pub async fn execute_nonstream(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut socket = connect_and_send(payload, req, model).await?;
    let mut text = String::new();
    let mut usage: Option<TokenUsage> = None;
    let mut saw_any_frame = false;
    let mut finish_reason = None;

    while let Some(message) = socket.next().await {
        let frame = parse_ws_message(message, model)?;
        if !frame.content_fragments.is_empty() || frame.done || frame.usage.is_some() {
            saw_any_frame = true;
        }
        for fragment in frame.content_fragments {
            text.push_str(&fragment);
        }
        if frame.usage.is_some() {
            usage = frame.usage;
        }
        if frame.done {
            finish_reason = Some("stop".to_string());
            break;
        }
    }

    if !saw_any_frame {
        return Err(GatewayError::service_unavailable(
            "XFYun native WebSocket upstream closed before returning any response frame",
        )
        .with_code("xfyun_websocket_empty_response")
        .with_provider("xfyun_websocket_compatible"));
    }

    Ok(CanonicalRelayResponse {
        model: model.to_string(),
        text,
        usage,
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason,
    })
}

pub async fn execute_stream_as_openai_sse(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<OpenAiSseByteStream, GatewayError> {
    let socket = connect_and_send(payload, req, model).await?;
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created_at = unix_timestamp_secs();
    let mut state = XfyunWsStreamState {
        socket,
        pending: VecDeque::new(),
        model: model.to_string(),
        response_id,
        created_at,
        finished: false,
    };
    prime_stream_state(&mut state).await?;

    let stream = futures::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok(bytes), state));
            }
            if state.finished {
                return None;
            }
            match state.socket.next().await {
                Some(message) => match message {
                    Ok(message) => match parse_ws_message(Ok(message), &state.model) {
                        Ok(parsed) => {
                            state.pending.extend(build_openai_sse_frames(
                                &parsed,
                                &state.model,
                                &state.response_id,
                                state.created_at,
                            ));
                            if parsed.done {
                                state.finished = true;
                            }
                        }
                        Err(error) => {
                            state.pending.push_back(Bytes::from(format_sse_event(
                                Some("error"),
                                &json!({
                                    "message": error.message,
                                    "code": error.code,
                                })
                                .to_string(),
                            )));
                            state
                                .pending
                                .push_back(Bytes::from_static(b"data: [DONE]\n\n"));
                            state.finished = true;
                        }
                    },
                    Err(_) => {
                        state.finished = true;
                    }
                },
                None => {
                    state.finished = true;
                }
            }
        }
    });

    Ok(Box::pin(stream))
}

fn pack_message(message: &CanonicalMessage) -> Option<Value> {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "user",
    };
    let content = render_message_text(message)?;
    Some(json!({
        "role": role,
        "content": content,
    }))
}

fn render_message_text(message: &CanonicalMessage) -> Option<String> {
    let mut fragments = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => {
                if !text.trim().is_empty() {
                    fragments.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let text = if let Some(text) = value.get("text").and_then(|entry| entry.as_str()) {
                    text.to_string()
                } else {
                    value.to_string()
                };
                if !text.trim().is_empty() {
                    fragments.push(text);
                }
            }
            ContentPart::ImageUrl { image_url, .. } => {
                fragments.push(format!("[image omitted: {image_url}]"));
            }
        }
    }
    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn read_required_app_id(payload: &ProviderAccountPayload) -> Result<String, GatewayError> {
    read_payload_string(payload.extra_body.as_ref(), &["appId", "app_id"]).ok_or_else(|| {
        GatewayError::unauthorized("XFYun native WebSocket provider missing APPID")
            .with_code("xfyun_websocket_missing_app_id")
            .with_provider("xfyun_websocket_compatible")
    })
}

fn read_required_api_secret(payload: &ProviderAccountPayload) -> Result<String, GatewayError> {
    payload
        .auth_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| read_payload_string(payload.extra_body.as_ref(), &["apiSecret", "api_secret"]))
        .ok_or_else(|| {
            GatewayError::unauthorized("XFYun native WebSocket provider missing APISecret")
                .with_code("xfyun_websocket_missing_api_secret")
                .with_provider("xfyun_websocket_compatible")
        })
}

fn websocket_path(payload: &ProviderAccountPayload) -> String {
    payload
        .chat_completions_path
        .clone()
        .or_else(|| {
            read_payload_string(
                payload.extra_body.as_ref(),
                &["wsPath", "websocketPath", "path"],
            )
        })
        .unwrap_or_else(|| XFYUN_WEBSOCKET_DEFAULT_PATH.to_string())
}

fn normalize_websocket_base_url(base_url: &str) -> Result<String, GatewayError> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(
            GatewayError::bad_request("XFYun native WebSocket provider missing base URL")
                .with_code("xfyun_websocket_missing_base_url")
                .with_provider("xfyun_websocket_compatible"),
        );
    }

    let normalized = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if trimmed.starts_with("wss://") || trimmed.starts_with("ws://") {
        trimmed.to_string()
    } else {
        format!("wss://{trimmed}")
    };

    Ok(normalized)
}

fn split_websocket_base(base_url: &str) -> Result<(String, String, String), GatewayError> {
    let (scheme, rest) = base_url.split_once("://").ok_or_else(|| {
        GatewayError::bad_request("XFYun native WebSocket base URL must include ws:// or wss://")
            .with_code("xfyun_websocket_invalid_base_url")
            .with_provider("xfyun_websocket_compatible")
    })?;
    let mut parts = rest.splitn(2, '/');
    let host = parts.next().unwrap_or_default().trim().to_string();
    if host.is_empty() {
        return Err(
            GatewayError::bad_request("XFYun native WebSocket base URL missing host")
                .with_code("xfyun_websocket_invalid_host")
                .with_provider("xfyun_websocket_compatible"),
        );
    }
    let path = parts
        .next()
        .map(|value| format!("/{}", value.trim_start_matches('/')))
        .unwrap_or_else(|| "/".to_string());
    Ok((scheme.to_string(), host, path))
}

fn join_paths(base_path: &str, path: &str) -> String {
    let normalized_base = if base_path.is_empty() { "/" } else { base_path };
    let normalized_path = if path.is_empty() {
        "/"
    } else if path.starts_with('/') {
        path
    } else {
        return format!("{}/{}", normalized_base.trim_end_matches('/'), path);
    };
    if normalized_base == "/" {
        normalized_path.to_string()
    } else {
        format!(
            "{}/{}",
            normalized_base.trim_end_matches('/'),
            normalized_path.trim_start_matches('/')
        )
    }
}

fn encode_query_component(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

async fn connect_and_send(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<XfyunWsSocket, GatewayError> {
    if !supports_endpoint(req.endpoint_kind) {
        return Err(
            GatewayError::bad_request(
                "XFYun native WebSocket adapters support only chat/messages/completions/responses style requests",
            )
            .with_code("unsupported_xfyun_websocket_endpoint")
            .with_provider("xfyun_websocket_compatible"),
        );
    }

    let signed_url = build_signed_websocket_url(payload)?;
    let request_body = pack_request(payload, req, model)?;
    let request_text = serde_json::to_string(&request_body).map_err(|error| {
        GatewayError::server_error(format!(
            "failed to serialize XFYun WebSocket request: {error}"
        ))
        .with_code("xfyun_websocket_request_serialize_failed")
        .with_provider("xfyun_websocket_compatible")
    })?;
    let (mut socket, _) = connect_xfyun_socket(&signed_url).await.map_err(|error| {
        GatewayError::service_unavailable(format!(
            "failed to connect to XFYun native WebSocket upstream: {error}"
        ))
        .with_code("xfyun_websocket_connect_failed")
        .with_provider("xfyun_websocket_compatible")
    })?;
    socket
        .send(Message::Text(request_text.into()))
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "failed to send XFYun native WebSocket request: {error}"
            ))
            .with_code("xfyun_websocket_send_failed")
            .with_provider("xfyun_websocket_compatible")
        })?;
    Ok(socket)
}

async fn connect_xfyun_socket(
    url: &str,
) -> Result<(XfyunWsSocket, WsResponse), tokio_tungstenite::tungstenite::Error> {
    if url.starts_with("wss://") {
        let request = url.into_client_request()?;
        let connector = Connector::Rustls(std::sync::Arc::new(build_rustls_client_config()));
        connect_async_tls_with_config(request, None, false, Some(connector)).await
    } else {
        connect_async(url).await
    }
}

fn build_rustls_client_config() -> ClientConfig {
    let mut root_store = RootCertStore::empty();
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        rustls_native_certs::load_native_certs();
    if !errors.is_empty() {
        warn!("native root CA certificate loading errors: {errors:?}");
    }
    let total_number = certs.len();
    let (number_added, number_ignored) = root_store.add_parsable_certificates(certs);
    debug!(
        "added {number_added}/{total_number} native root certificates for XFYun WebSocket connector (ignored {number_ignored})"
    );
    ClientConfig::builder_with_provider(std::sync::Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("ring provider should expose a usable TLS version set")
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

async fn prime_stream_state(state: &mut XfyunWsStreamState) -> Result<(), GatewayError> {
    while state.pending.is_empty() && !state.finished {
        let Some(message) = state.socket.next().await else {
            return Err(GatewayError::service_unavailable(
                "XFYun native WebSocket upstream closed before the first response frame",
            )
            .with_code("xfyun_websocket_closed_early")
            .with_provider("xfyun_websocket_compatible"));
        };
        let parsed = parse_ws_message(message, &state.model)?;
        state.pending.extend(build_openai_sse_frames(
            &parsed,
            &state.model,
            &state.response_id,
            state.created_at,
        ));
        if parsed.done {
            state.finished = true;
        }
    }
    Ok(())
}

fn parse_ws_message(
    message: Result<Message, tokio_tungstenite::tungstenite::Error>,
    model: &str,
) -> Result<ParsedXfyunFrame, GatewayError> {
    match message {
        Ok(Message::Text(text)) => parse_frame_text(text.as_ref(), model),
        Ok(Message::Binary(bytes)) => {
            let text = String::from_utf8(bytes.to_vec()).map_err(|error| {
                GatewayError::service_unavailable(format!(
                    "XFYun native WebSocket returned invalid UTF-8 frame: {error}"
                ))
                .with_code("xfyun_websocket_invalid_utf8")
                .with_provider("xfyun_websocket_compatible")
            })?;
            parse_frame_text(&text, model)
        }
        Ok(Message::Close(_)) => Ok(ParsedXfyunFrame {
            done: true,
            ..ParsedXfyunFrame::default()
        }),
        Ok(Message::Ping(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {
            Ok(ParsedXfyunFrame::default())
        }
        Err(error) => Err(GatewayError::service_unavailable(format!(
            "XFYun native WebSocket transport error: {error}"
        ))
        .with_code("xfyun_websocket_transport_failed")
        .with_provider("xfyun_websocket_compatible")),
    }
}

fn parse_frame_text(text: &str, _model: &str) -> Result<ParsedXfyunFrame, GatewayError> {
    let value: Value = serde_json::from_str(text).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "XFYun native WebSocket returned invalid JSON frame: {error}"
        ))
        .with_code("xfyun_websocket_invalid_json")
        .with_provider("xfyun_websocket_compatible")
    })?;
    let code = value
        .pointer("/header/code")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(0);
    if code != 0 {
        let message = value
            .pointer("/header/message")
            .and_then(|entry| entry.as_str())
            .unwrap_or("unknown upstream error");
        return Err(classify_protocol_error(code, message));
    }

    let mut content_fragments = Vec::new();
    if let Some(items) = value
        .pointer("/payload/choices/text")
        .and_then(|entry| entry.as_array())
    {
        for item in items {
            if let Some(content) = item.get("content").and_then(|entry| entry.as_str()) {
                if !content.is_empty() {
                    content_fragments.push(content.to_string());
                }
            }
        }
    }

    let status = value
        .pointer("/payload/choices/status")
        .and_then(|entry| entry.as_u64())
        .unwrap_or(0);
    let usage = parse_usage(value.pointer("/payload/usage/text"));

    if content_fragments.is_empty() && status == 0 && usage.is_none() {
        // Native XFYun WebSocket surfaces can emit transient empty status=0
        // frames before the first text delta. Treat those as no-op keepalive
        // frames instead of aborting the caller-visible stream.
        return Ok(ParsedXfyunFrame::default());
    }

    Ok(ParsedXfyunFrame {
        content_fragments,
        usage,
        done: status == 2,
    })
}

fn classify_protocol_error(code: i64, message: &str) -> GatewayError {
    let normalized = message.to_ascii_lowercase();
    let error = if normalized.contains("auth")
        || normalized.contains("signature")
        || normalized.contains("apikey")
        || normalized.contains("api key")
        || normalized.contains("secret")
        || normalized.contains("appid")
        || normalized.contains("permission")
    {
        GatewayError::unauthorized(format!("XFYun native WebSocket auth failed: {message}"))
            .with_code(format!("xfyun_websocket_{code}"))
    } else if normalized.contains("qps")
        || normalized.contains("rate")
        || normalized.contains("limit")
        || normalized.contains("too many")
    {
        GatewayError::rate_limited(
            format!("XFYun native WebSocket rate limited: {message}"),
            1000,
        )
        .with_code(format!("xfyun_websocket_{code}"))
    } else {
        GatewayError::service_unavailable(format!(
            "XFYun native WebSocket upstream error {code}: {message}"
        ))
        .with_code(format!("xfyun_websocket_{code}"))
    };
    error.with_provider("xfyun_websocket_compatible")
}

fn parse_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let value = value?;
    let prompt_tokens =
        read_usage_field(value, &["prompt_tokens", "question_tokens", "input_tokens"])?;
    let completion_tokens = read_usage_field(
        value,
        &["completion_tokens", "answer_tokens", "output_tokens"],
    )
    .unwrap_or(0);
    let total_tokens =
        read_usage_field(value, &["total_tokens"]).unwrap_or(prompt_tokens + completion_tokens);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

fn read_usage_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|entry| entry.as_u64()))
}

fn build_openai_sse_frames(
    parsed: &ParsedXfyunFrame,
    model: &str,
    response_id: &str,
    created_at: i64,
) -> VecDeque<Bytes> {
    let mut frames = VecDeque::new();
    for fragment in &parsed.content_fragments {
        let payload = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created_at,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": fragment,
                },
                "finish_reason": Value::Null,
            }],
        });
        frames.push_back(Bytes::from(format_sse_event(None, &payload.to_string())));
    }

    if parsed.done {
        let mut payload = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created_at,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop",
            }],
        });
        if let Some(usage) = &parsed.usage {
            payload["usage"] = json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            });
        }
        frames.push_back(Bytes::from(format_sse_event(None, &payload.to_string())));
        frames.push_back(Bytes::from_static(b"data: [DONE]\n\n"));
    }

    frames
}

fn request_value<'a>(req: &'a CanonicalRelayRequest, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = req.raw_body.get(*key) {
            return Some(value);
        }
        if let Some(value) = req.extra.get(*key) {
            return Some(value);
        }
    }
    None
}

fn normalize_parameter_field_name(key: &str) -> Option<&'static str> {
    match key.to_ascii_lowercase().as_str() {
        "temperature" => Some("temperature"),
        "top_p" | "topp" => Some("top_p"),
        "top_k" | "topk" => Some("top_k"),
        "max_tokens" | "maxoutputtokens" | "max_output_tokens" => Some("max_tokens"),
        _ => None,
    }
}

fn read_payload_string(extra: Option<&HashMap<String, Value>>, keys: &[&str]) -> Option<String> {
    let extra = extra?;
    keys.iter().find_map(|key| {
        extra
            .get(*key)
            .and_then(|entry| entry.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn read_payload_array(extra: Option<&HashMap<String, Value>>, keys: &[&str]) -> Option<Value> {
    let extra = extra?;
    keys.iter()
        .find_map(|key| extra.get(*key))
        .and_then(|entry| entry.as_array().cloned())
        .map(Value::Array)
}

fn unix_timestamp_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "xfyun_websocket_compatible".to_string(),
            base_url: "wss://maas-api.cn-huabei-1.xf-yun.com".to_string(),
            api_key: "api-key-123".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some("xop35qwen2b".to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: Some("api-secret-456".to_string()),
            responses_path: None,
            chat_completions_path: Some("/v1.1/chat".to_string()),
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(HashMap::from([
                ("appId".to_string(), json!("app-id-789")),
                ("uid".to_string(), json!("gateway-test")),
            ])),
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("qwen3.5-35b-a3b".to_string()),
            stream: true,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Reply with exactly: native websocket ok".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "temperature": 0.1,
                "max_tokens": 256,
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn packs_native_request_body() {
        let body = pack_request(&make_payload(), &make_request(), "xop35qwen2b").unwrap();
        assert_eq!(body["header"]["app_id"], "app-id-789");
        assert_eq!(body["parameter"]["chat"]["domain"], "xop35qwen2b");
        assert_eq!(body["parameter"]["chat"]["temperature"], json!(0.1));
        assert_eq!(body["parameter"]["chat"]["max_tokens"], json!(256));
        assert_eq!(body["payload"]["message"]["text"][0]["role"], "system");
        assert_eq!(body["payload"]["message"]["text"][1]["role"], "user");
    }

    #[test]
    fn builds_signed_websocket_url() {
        let url = build_signed_websocket_url(&make_payload()).unwrap();
        assert!(url.starts_with("wss://maas-api.cn-huabei-1.xf-yun.com/v1.1/chat?"));
        assert!(url.contains("authorization="));
        assert!(url.contains("date="));
        assert!(url.contains("host="));
    }

    fn assert_request_plan_contract(plan: &RequestPlan) {
        assert_eq!(plan.method, Method::POST);
        assert!(plan
            .url
            .starts_with("wss://maas-api.cn-huabei-1.xf-yun.com/v1.1/chat?"));
        assert!(plan.url.contains("authorization="));
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert_eq!(
            plan.body.as_ref().unwrap()["header"]["app_id"],
            "app-id-789"
        );
        assert_eq!(
            plan.body.as_ref().unwrap()["parameter"]["chat"]["domain"],
            "xop35qwen2b"
        );
    }

    #[test]
    fn xfyun_websocket_request_plan_builds_signed_url_and_native_body() {
        let plan = build_request_plan(&make_payload(), &make_request(), "xop35qwen2b").unwrap();
        assert_request_plan_contract(&plan);
    }

    #[test]
    fn xfyun_websocket_request_plan_rejects_unsupported_endpoint() {
        let mut req = make_request();
        req.endpoint_kind = EndpointKind::Embeddings;
        let err = build_request_plan(&make_payload(), &req, "xop35qwen2b")
            .expect_err("xfyun websocket should reject embeddings");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_xfyun_websocket_endpoint")
        );
    }

    #[test]
    fn parses_success_frame() {
        let parsed = parse_frame_text(
            r#"{
                "header": {"code": 0},
                "payload": {
                    "choices": {
                        "status": 2,
                        "text": [{"content": "hello websocket"}]
                    },
                    "usage": {
                        "text": {
                            "prompt_tokens": 8,
                            "completion_tokens": 4,
                            "total_tokens": 12
                        }
                    }
                }
            }"#,
            "xop35qwen2b",
        )
        .unwrap();
        assert_eq!(
            parsed.content_fragments,
            vec!["hello websocket".to_string()]
        );
        assert!(parsed.done);
        assert_eq!(
            parsed.usage.as_ref().map(|usage| usage.total_tokens),
            Some(12)
        );
    }

    #[test]
    fn ignores_empty_intermediate_frame() {
        let parsed = parse_frame_text(
            r#"{
                "header": {"code": 0},
                "payload": {
                    "choices": {
                        "status": 0,
                        "text": []
                    }
                }
            }"#,
            "xop35qwen2b",
        )
        .unwrap();
        assert!(parsed.content_fragments.is_empty());
        assert!(parsed.usage.is_none());
        assert!(!parsed.done);
    }

    #[test]
    fn converts_frame_to_openai_sse() {
        let parsed = ParsedXfyunFrame {
            content_fragments: vec!["delta".to_string()],
            usage: Some(TokenUsage {
                prompt_tokens: 3,
                completion_tokens: 2,
                total_tokens: 5,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            done: true,
        };
        let frames = build_openai_sse_frames(&parsed, "xop35qwen2b", "chatcmpl-test", 123);
        let joined = frames
            .into_iter()
            .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
            .collect::<Vec<_>>()
            .join("");
        assert!(joined.contains("\"chatcmpl-test\""));
        assert!(joined.contains("\"delta\""));
        assert!(joined.contains("\"finish_reason\":\"stop\""));
        assert!(joined.contains("data: [DONE]"));
    }
}
