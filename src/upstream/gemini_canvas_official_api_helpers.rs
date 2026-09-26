use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use futures::{SinkExt, StreamExt};
use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::{Client, Method};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::web_reverse as gemini_web_reverse_modular;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasRuntimeApiContext;
use crate::upstream::gemini_canvas_direct_http_helpers::redact_gemini_canvas_api_key_for_logs;
use crate::upstream::gemini_canvas_music_helpers::{
    build_gemini_canvas_music_ws_client_content_frame,
    build_gemini_canvas_music_ws_generation_config_frame,
    build_gemini_canvas_music_ws_playback_control_frame, build_gemini_canvas_music_ws_setup_frame,
    consume_gemini_canvas_music_server_content, gemini_canvas_music_filtered_prompt_error,
    gemini_canvas_music_missing_api_key_error, gemini_canvas_music_socket_url_with_api_key,
    gemini_canvas_music_ws_authenticated_setup_error, gemini_canvas_music_ws_connect_failed_error,
    gemini_canvas_music_ws_missing_audio_error, gemini_canvas_music_ws_pong_failed_error,
    gemini_canvas_music_ws_request_failed_error, gemini_canvas_music_ws_send_frame_error,
    gemini_canvas_music_ws_setup_timeout_error, gemini_canvas_music_ws_setup_transport_error,
    gemini_canvas_music_ws_stream_transport_error, parse_gemini_canvas_music_ws_binary,
    parse_gemini_canvas_music_ws_setup_binary, parse_gemini_canvas_music_ws_setup_text,
    parse_gemini_canvas_music_ws_text,
};
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_page_context_headers, apply_gemini_canvas_signed_headers,
};
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::response_preview_helpers::compact_response_preview;

#[cfg(test)]
pub(crate) fn gemini_canvas_official_api_enabled(payload: &ProviderAccountPayload) -> bool {
    !payload.api_key.trim().is_empty()
}

pub(crate) fn build_gemini_canvas_official_headers(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
) -> HeaderMap {
    let mut api_payload = payload.clone();
    api_payload.adapter = "gemini_api_compatible".to_string();
    build_upstream_headers_with(&api_payload, extra_headers)
}

pub(crate) fn parse_gemini_canvas_official_json_body(
    body_text: &str,
) -> Result<Value, GatewayError> {
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas official API returned invalid JSON: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_official_invalid_json")
    })
}

pub(crate) async fn connect_gemini_canvas_music_socket(
    payload: &ProviderAccountPayload,
    runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
    extra_headers: Option<&HashMap<String, String>>,
    socket_url_override: Option<&str>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    GatewayError,
> {
    let api_key = payload.api_key.trim();
    if api_key.is_empty() {
        return Err(gemini_canvas_music_missing_api_key_error());
    }
    let socket_url = socket_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        );
    let url = gemini_canvas_music_socket_url_with_api_key(socket_url, api_key)?;
    let mut request = url.as_str().into_client_request().map_err(|error| {
        gemini_canvas_music_ws_request_failed_error(
            "build Gemini Canvas music websocket request",
            error.to_string().as_str(),
        )
    })?;
    let base_headers = build_gemini_canvas_official_headers(payload, extra_headers);
    for (name, value) in base_headers.iter() {
        request.headers_mut().insert(name, value.clone());
    }
    if let Some(runtime_api) = runtime_api {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        let authorization = gemini_canvas::build_sapisid_authorization(
            &runtime_api.session.sapisid,
            &runtime_api.page_origin,
            timestamp_secs,
        )?;
        let _ = apply_gemini_canvas_page_context_headers(
            request.headers_mut(),
            &runtime_api.page_origin,
            &runtime_api.page_referer,
            "https://generativelanguage.googleapis.com",
            true,
        );
        apply_gemini_canvas_signed_headers(
            request.headers_mut(),
            &runtime_api.session,
            &runtime_api.page_origin,
            &runtime_api.page_referer,
            &authorization,
            true,
        );
    }
    request.headers_mut().insert(
        HeaderName::from_static("x-goog-api-key"),
        HeaderValue::from_str(api_key).map_err(|error| {
            gemini_canvas_music_ws_request_failed_error(
                "encode Gemini Canvas music api_key",
                error.to_string().as_str(),
            )
        })?,
    );
    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| gemini_canvas_music_ws_connect_failed_error(error.to_string().as_str()))?;
    Ok(socket)
}

pub(crate) async fn execute_gemini_canvas_official_music(
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
    extra_headers: Option<&HashMap<String, String>>,
    socket_url_override: Option<&str>,
    prompt_override: Option<&str>,
    _duration_seconds_override: Option<f64>,
    upstream_model_override: Option<&str>,
) -> Result<Value, GatewayError> {
    let upstream_model_owned;
    let upstream_model = if let Some(override_model) = upstream_model_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        upstream_model_owned = override_model.to_string();
        upstream_model_owned.as_str()
    } else {
        gemini_canvas::resolve_official_music_model(model)?
    };
    let prompt = prompt_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or(
            gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                req,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )?,
        );
    let api_key_candidates = if let Some(runtime_api) = runtime_api {
        let mut keys = Vec::new();
        for candidate in &runtime_api.api_key_candidates {
            let trimmed = candidate.trim();
            if trimmed.is_empty() || keys.iter().any(|existing: &String| existing == trimmed) {
                continue;
            }
            keys.push(trimmed.to_string());
        }
        if keys.is_empty() {
            keys.push(payload.api_key.trim().to_string());
        }
        keys
    } else {
        vec![payload.api_key.trim().to_string()]
    };
    let candidate_count = api_key_candidates.len();
    let mut last_setup_error: Option<GatewayError> = None;
    let mut established_socket = None;

    for (candidate_index, api_key_candidate) in api_key_candidates.iter().enumerate() {
        let mut attempt_payload = payload.clone();
        attempt_payload.api_key = api_key_candidate.clone();
        debug!(
            provider = "gemini_canvas_compatible",
            candidate_index = candidate_index + 1,
            candidate_count,
            api_key = %redact_gemini_canvas_api_key_for_logs(api_key_candidate),
            "trying Gemini Canvas music websocket api key candidate"
        );
        let mut socket = match connect_gemini_canvas_music_socket(
            &attempt_payload,
            runtime_api,
            extra_headers,
            socket_url_override,
        )
        .await
        {
            Ok(socket) => socket,
            Err(error) => {
                last_setup_error = Some(error);
                continue;
            }
        };
        if let Err(error) = socket
            .send(Message::Text(
                build_gemini_canvas_music_ws_setup_frame(upstream_model).into(),
            ))
            .await
            .map_err(|error| {
                gemini_canvas_music_ws_send_frame_error("music setup", error.to_string().as_str())
            })
        {
            last_setup_error = Some(error);
            continue;
        }

        let mut setup_complete = false;
        let mut last_setup_event: Option<String> = None;
        let setup_deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < setup_deadline {
            let Some(message) = socket.next().await else {
                break;
            };
            let value = match message {
                Ok(Message::Text(text)) => {
                    last_setup_event = Some(compact_response_preview(text.as_ref(), 240));
                    parse_gemini_canvas_music_ws_setup_text(text.as_ref())?
                }
                Ok(Message::Binary(bytes)) => {
                    last_setup_event = Some(compact_response_preview(
                        &String::from_utf8_lossy(&bytes),
                        240,
                    ));
                    parse_gemini_canvas_music_ws_setup_binary(&bytes)?
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
                Ok(Message::Ping(payload)) => {
                    socket.send(Message::Pong(payload)).await.map_err(|error| {
                        gemini_canvas_music_ws_pong_failed_error(error.to_string().as_str())
                    })?;
                    continue;
                }
                Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => continue,
                Err(error) => {
                    let transport_error =
                        gemini_canvas_music_ws_setup_transport_error(error.to_string().as_str());
                    last_setup_error = Some(transport_error);
                    break;
                }
            };
            if value.get("setupComplete").is_some() || value.get("setup_complete").is_some() {
                setup_complete = true;
                break;
            }
        }
        if setup_complete {
            established_socket = Some(socket);
            break;
        }
        let error = gemini_canvas_music_ws_setup_timeout_error(last_setup_event);
        debug!(
            provider = "gemini_canvas_compatible",
            candidate_index = candidate_index + 1,
            candidate_count,
            api_key = %redact_gemini_canvas_api_key_for_logs(api_key_candidate),
            message = %error.message,
            code = ?error.code,
            "Gemini Canvas music websocket candidate failed during setup"
        );
        last_setup_error = Some(error);
    }

    let mut socket = match established_socket {
        Some(socket) => socket,
        None => {
            return Err(
                last_setup_error.unwrap_or_else(gemini_canvas_music_ws_authenticated_setup_error)
            );
        }
    };

    socket
        .send(Message::Text(
            build_gemini_canvas_music_ws_client_content_frame(&prompt).into(),
        ))
        .await
        .map_err(|error| {
            gemini_canvas_music_ws_send_frame_error(
                "music client_content",
                error.to_string().as_str(),
            )
        })?;

    let music_generation_config = gemini_canvas::build_music_generation_config(req);
    if music_generation_config
        .as_object()
        .map(|config| !config.is_empty())
        .unwrap_or(false)
    {
        socket
            .send(Message::Text(
                build_gemini_canvas_music_ws_generation_config_frame(&music_generation_config)
                    .into(),
            ))
            .await
            .map_err(|error| {
                gemini_canvas_music_ws_send_frame_error(
                    "music_generation_config",
                    error.to_string().as_str(),
                )
            })?;
    }

    socket
        .send(Message::Text(
            build_gemini_canvas_music_ws_playback_control_frame().into(),
        ))
        .await
        .map_err(|error| {
            gemini_canvas_music_ws_send_frame_error("playback_control", error.to_string().as_str())
        })?;

    let deadline = Instant::now() + timeout.max(Duration::from_secs(60));
    let mut saw_audio = false;
    let mut audio_mime_type = String::from("audio/L16;codec=pcm;rate=48000;channels=2");
    let mut audio_bytes = Vec::new();
    while Instant::now() < deadline {
        let idle_timeout = if saw_audio {
            Duration::from_secs(3)
        } else {
            Duration::from_secs(20)
        };
        let wait_for = idle_timeout.min(deadline.saturating_duration_since(Instant::now()));
        match tokio::time::timeout(wait_for, socket.next()).await {
            Err(_) if saw_audio => break,
            Err(_) => continue,
            Ok(None) => break,
            Ok(Some(Err(error))) => {
                return Err(gemini_canvas_music_ws_stream_transport_error(
                    error.to_string().as_str(),
                ));
            }
            Ok(Some(Ok(Message::Close(_)))) => break,
            Ok(Some(Ok(Message::Ping(payload)))) => {
                socket.send(Message::Pong(payload)).await.map_err(|error| {
                    gemini_canvas_music_ws_pong_failed_error(error.to_string().as_str())
                })?;
            }
            Ok(Some(Ok(Message::Pong(_)))) | Ok(Some(Ok(Message::Frame(_)))) => {}
            Ok(Some(Ok(Message::Binary(bytes)))) => {
                let value = parse_gemini_canvas_music_ws_binary(&bytes)?;
                saw_audio |= consume_gemini_canvas_music_server_content(
                    &value,
                    &mut audio_mime_type,
                    &mut audio_bytes,
                )?;
            }
            Ok(Some(Ok(Message::Text(text)))) => {
                let value = parse_gemini_canvas_music_ws_text(text.as_ref())?;
                if let Some(filtered_prompt) = value
                    .get("filteredPrompt")
                    .or_else(|| value.get("filtered_prompt"))
                {
                    return Err(gemini_canvas_music_filtered_prompt_error(filtered_prompt));
                }
                saw_audio |= consume_gemini_canvas_music_server_content(
                    &value,
                    &mut audio_mime_type,
                    &mut audio_bytes,
                )?;
            }
        }
    }

    if audio_bytes.is_empty() {
        return Err(gemini_canvas_music_ws_missing_audio_error());
    }

    let audio = gemini_canvas::GeminiCanvasAudio {
        mime_type: audio_mime_type,
        bytes: audio_bytes,
    };
    let (body, content_type) = gemini_canvas::build_audio_binary_response(req, &audio)?;
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(&body);
    let asset = gemini_canvas::GeminiCanvasMediaAsset {
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
    Ok(gemini_canvas::build_music_generation_response(
        model, &prompt, &asset, None,
    ))
}

pub(crate) async fn send_gemini_canvas_official_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    body: &Value,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let headers = build_gemini_canvas_official_headers(payload, extra_headers);
    let response = http
        .request(Method::POST, url)
        .headers(headers)
        .timeout(timeout)
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("gemini_canvas_compatible"),
        ));
    }

    parse_gemini_canvas_official_json_body(&body_text)
}

pub(crate) async fn send_gemini_canvas_official_get_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let headers = build_gemini_canvas_official_headers(payload, extra_headers);
    let response = http
        .request(Method::GET, url)
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some("gemini_canvas_compatible"),
        ));
    }

    parse_gemini_canvas_official_json_body(&body_text)
}

pub(crate) async fn send_gemini_canvas_official_get_bytes(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
    let headers = build_gemini_canvas_official_headers(payload, extra_headers);
    let response = http
        .request(Method::GET, url)
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .bytes()
        .await
        .map_err(|error| classify_network_error(&error, Some("gemini_canvas_compatible")))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &String::from_utf8_lossy(&bytes),
            Some("gemini_canvas_compatible"),
        ));
    }

    Ok((bytes, content_type))
}

mod video;

pub(crate) use video::execute_gemini_canvas_official_video;

#[cfg(test)]
mod tests;
