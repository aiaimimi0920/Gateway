//! Official music WebSocket connection, setup, and streaming lifetime.

use std::collections::HashMap;
use std::time::Duration;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini::api as surface;
use crate::protocol::gemini_canvas as legacy;
use crate::routing::candidate::ProviderAccountPayload;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use rquest::header::{HeaderName, HeaderValue};
use rquest::Client;
use serde_json::{json, Value};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::super::execution::GEMINI_API_LEGACY_ADAPTER;
use super::{prompt_from_media_request, transport::build_official_headers};

mod frames;

use frames::{compact_response_preview, consume_music_server_content};

const DEFAULT_MUSIC_WS_URL: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic";

pub async fn execute_official_music(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    socket_url_override: Option<&str>,
) -> Result<Value, GatewayError> {
    let upstream_model = surface::resolve_official_music_model(model)?;
    let prompt = prompt_from_media_request(
        req,
        "Gemini official music generation requires a prompt.",
        "missing_gemini_official_music_prompt",
    )?;
    let mut socket =
        connect_official_music_socket(http, payload, extra_headers, socket_url_override).await?;
    socket
        .send(Message::Text(
            json!({
                "setup": {
                    "model": format!("models/{upstream_model}")
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "failed to send Gemini official music setup frame: {error}"
            ))
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_music_ws_send_failed")
        })?;

    let mut setup_complete = false;
    let mut last_setup_event: Option<String> = None;
    let setup_deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < setup_deadline {
        let Some(message) = socket.next().await else {
            break;
        };
        let value = match message {
            Ok(Message::Text(text)) => {
                last_setup_event = Some(compact_response_preview(text.as_ref(), 240));
                serde_json::from_str::<Value>(text.as_ref()).map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "Gemini official music websocket returned invalid JSON during setup: {error}"
                    ))
                    .with_provider(payload.adapter.as_str())
                    .with_code("gemini_official_music_ws_invalid_json")
                })?
            }
            Ok(Message::Binary(bytes)) => {
                last_setup_event = Some(compact_response_preview(
                    &String::from_utf8_lossy(&bytes),
                    240,
                ));
                serde_json::from_slice::<Value>(&bytes).map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "Gemini official music websocket returned invalid binary JSON during setup: {error}"
                    ))
                    .with_provider(payload.adapter.as_str())
                    .with_code("gemini_official_music_ws_invalid_json")
                })?
            }
            Ok(Message::Close(frame)) => {
                last_setup_event = Some(match frame {
                    Some(frame) if !frame.reason.is_empty() => {
                        format!("close code={} reason={}", frame.code, frame.reason)
                    }
                    Some(frame) => format!("close code={}", frame.code),
                    None => "close".to_string(),
                });
                break;
            }
            Ok(Message::Ping(payload_bytes)) => {
                socket
                    .send(Message::Pong(payload_bytes))
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!(
                            "failed to reply to Gemini official music websocket ping: {error}"
                        ))
                        .with_provider(payload.adapter.as_str())
                        .with_code("gemini_official_music_ws_send_failed")
                    })?;
                continue;
            }
            Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => continue,
            Err(error) => {
                return Err(GatewayError::service_unavailable(format!(
                    "Gemini official music websocket setup failed: {error}"
                ))
                .with_provider(payload.adapter.as_str())
                .with_code("gemini_official_music_ws_transport_failed"));
            }
        };
        if value.get("setupComplete").is_some() || value.get("setup_complete").is_some() {
            setup_complete = true;
            break;
        }
    }
    if !setup_complete {
        let mut error = GatewayError::service_unavailable(
            "Gemini official music websocket did not acknowledge setup.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_music_ws_setup_timeout");
        if let Some(event) = last_setup_event {
            error.message = format!("{}; last_setup_event={event}", error.message);
        }
        return Err(error);
    }

    socket
        .send(Message::Text(
            json!({
                "client_content": surface::build_music_client_content(&prompt)
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "failed to send Gemini official music client_content frame: {error}"
            ))
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_music_ws_send_failed")
        })?;

    let music_generation_config = surface::build_music_generation_config(req);
    if music_generation_config
        .as_object()
        .map(|config| !config.is_empty())
        .unwrap_or(false)
    {
        socket
            .send(Message::Text(
                json!({
                    "music_generation_config": music_generation_config
                })
                .to_string()
                .into(),
            ))
            .await
            .map_err(|error| {
                GatewayError::service_unavailable(format!(
                    "failed to send Gemini official music_generation_config frame: {error}"
                ))
                .with_provider(payload.adapter.as_str())
                .with_code("gemini_official_music_ws_send_failed")
            })?;
    }

    socket
        .send(Message::Text(
            json!({
                "playback_control": "PLAY"
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "failed to send Gemini official playback_control frame: {error}"
            ))
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_music_ws_send_failed")
        })?;

    let deadline = std::time::Instant::now() + timeout.max(Duration::from_secs(60));
    let mut saw_audio = false;
    let mut audio_mime_type = String::from("audio/L16;codec=pcm;rate=48000;channels=2");
    let mut audio_bytes = Vec::new();
    while std::time::Instant::now() < deadline {
        let idle_timeout = if saw_audio {
            Duration::from_secs(3)
        } else {
            Duration::from_secs(20)
        };
        let wait_for =
            idle_timeout.min(deadline.saturating_duration_since(std::time::Instant::now()));
        match tokio::time::timeout(wait_for, socket.next()).await {
            Err(_) if saw_audio => break,
            Err(_) => continue,
            Ok(None) => break,
            Ok(Some(Err(error))) => {
                return Err(GatewayError::service_unavailable(format!(
                    "Gemini official music websocket failed while streaming audio: {error}"
                ))
                .with_provider(payload.adapter.as_str())
                .with_code("gemini_official_music_ws_transport_failed"));
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(Message::Ping(payload_bytes)))) => {
                socket
                    .send(Message::Pong(payload_bytes))
                    .await
                    .map_err(|error| {
                        GatewayError::service_unavailable(format!(
                            "failed to reply to Gemini official music websocket ping: {error}"
                        ))
                        .with_provider(payload.adapter.as_str())
                        .with_code("gemini_official_music_ws_send_failed")
                    })?;
            }
            Ok(Some(Ok(Message::Pong(_)))) | Ok(Some(Ok(Message::Frame(_)))) => {}
            Ok(Some(Ok(Message::Binary(bytes)))) => {
                let value = serde_json::from_slice::<Value>(&bytes).map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "Gemini official music websocket returned invalid binary JSON: {error}"
                    ))
                    .with_provider(payload.adapter.as_str())
                    .with_code("gemini_official_music_ws_invalid_json")
                })?;
                saw_audio |=
                    consume_music_server_content(&value, &mut audio_mime_type, &mut audio_bytes)?;
            }
            Ok(Some(Ok(Message::Text(text)))) => {
                let value = serde_json::from_str::<Value>(text.as_ref()).map_err(|error| {
                    GatewayError::service_unavailable(format!(
                        "Gemini official music websocket returned invalid JSON: {error}"
                    ))
                    .with_provider(payload.adapter.as_str())
                    .with_code("gemini_official_music_ws_invalid_json")
                })?;
                if let Some(filtered_prompt) = value
                    .get("filteredPrompt")
                    .or_else(|| value.get("filtered_prompt"))
                {
                    return Err(GatewayError::bad_request(format!(
                        "Gemini official music prompt was filtered: {filtered_prompt}"
                    ))
                    .with_provider(payload.adapter.as_str())
                    .with_code("gemini_official_music_filtered_prompt"));
                }
                saw_audio |=
                    consume_music_server_content(&value, &mut audio_mime_type, &mut audio_bytes)?;
            }
        }
    }

    if audio_bytes.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini official music websocket completed without audio chunks.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_music_missing_audio"));
    }

    let audio = legacy::GeminiCanvasAudio {
        mime_type: audio_mime_type,
        bytes: audio_bytes,
    };
    let (body, content_type) = surface::build_audio_binary_response(req, &audio)?;
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(&body);
    let asset = legacy::GeminiCanvasMediaAsset {
        kind: "audio".to_string(),
        url: format!("data:{content_type};base64,{body_base64}"),
        mime_type: content_type,
        download_token: None,
        body_base64: Some(body_base64),
        alt: Some(prompt.clone()),
        width: None,
        height: None,
        duration_seconds: None,
    };
    Ok(surface::build_music_generation_response(
        model, &prompt, &asset, None,
    ))
}

async fn connect_official_music_socket(
    http: &Client,
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
    socket_url_override: Option<&str>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    GatewayError,
> {
    let api_key = payload.api_key.trim();
    if api_key.is_empty() {
        return Err(
            GatewayError::unauthorized("Gemini official music API requires api_key.")
                .with_provider(payload.adapter.as_str())
                .with_code("gemini_official_missing_api_key"),
        );
    }
    let socket_url = socket_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_MUSIC_WS_URL);
    let url = music_socket_url_with_api_key(socket_url, api_key)?;
    let mut request = url.as_str().into_client_request().map_err(|error| {
        GatewayError::server_error(format!(
            "failed to build Gemini official music websocket request: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_music_ws_request_failed")
    })?;
    let base_headers = build_official_headers(payload, extra_headers);
    for (name, value) in base_headers.iter() {
        request.headers_mut().insert(name, value.clone());
    }
    request.headers_mut().insert(
        HeaderName::from_static("x-goog-api-key"),
        HeaderValue::from_str(api_key).map_err(|error| {
            GatewayError::server_error(format!(
                "failed to encode Gemini official music api_key: {error}"
            ))
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_music_ws_request_failed")
        })?,
    );
    let (socket, _) = connect_async(request).await.map_err(|error| {
        GatewayError::service_unavailable(format!(
            "failed to connect to Gemini official music websocket: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_music_ws_connect_failed")
    })?;
    let _ = http;
    Ok(socket)
}

fn music_socket_url_with_api_key(socket_url: &str, api_key: &str) -> Result<String, GatewayError> {
    let trimmed_socket_url = socket_url.trim();
    let trimmed_api_key = api_key.trim();
    if trimmed_socket_url.is_empty() || trimmed_api_key.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini official music websocket requires api_key.",
        )
        .with_provider(GEMINI_API_LEGACY_ADAPTER)
        .with_code("gemini_official_missing_api_key"));
    }
    let mut parsed = url::Url::parse(trimmed_socket_url).map_err(|error| {
        GatewayError::server_error(format!(
            "failed to parse Gemini official music websocket URL: {error}"
        ))
        .with_provider(GEMINI_API_LEGACY_ADAPTER)
        .with_code("gemini_official_music_ws_request_failed")
    })?;
    if !parsed
        .query_pairs()
        .any(|(key, value)| key == "key" && !value.trim().is_empty())
    {
        parsed.query_pairs_mut().append_pair("key", trimmed_api_key);
    }
    Ok(parsed.to_string())
}
