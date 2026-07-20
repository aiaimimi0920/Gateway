use std::collections::HashMap;
use std::time::Duration;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::gemini::api as surface;
use crate::protocol::gemini_canvas as legacy;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::response_types::BinaryUpstreamResponse;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::{Client, Method};
use serde_json::{json, Value};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::execution::GEMINI_API_LEGACY_ADAPTER;

const DEFAULT_MUSIC_WS_URL: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic";

pub fn supports_media_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::AudioSpeech
            | EndpointKind::ImagesGenerations
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
    )
}

pub async fn execute_official_tts(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    let upstream_model = if model.trim().is_empty() {
        legacy::GEMINI_CANVAS_OFFICIAL_TTS_MODEL
    } else {
        model
    };
    let request_body = surface::build_tts_request_body(req, upstream_model);
    let request_url = format!(
        "{}/models/{}:generateContent",
        payload.base_url.trim_end_matches('/'),
        upstream_model
    );
    let body = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let audio = surface::extract_audio_from_generate_content_response(&body)?;
    let (bytes, content_type) = surface::build_audio_binary_response(req, &audio)?;
    Ok(BinaryUpstreamResponse {
        body: bytes::Bytes::from(bytes),
        content_type: Some(content_type),
        extra_headers: Vec::new(),
    })
}

pub async fn execute_official_media(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ImagesGenerations => {
            execute_official_image(http, timeout, payload, req, model, extra_headers).await
        }
        EndpointKind::MusicGenerations => {
            execute_official_music(http, timeout, payload, req, model, extra_headers, None).await
        }
        EndpointKind::VideosGenerations => {
            execute_official_video(http, timeout, payload, req, model, extra_headers, None).await
        }
        EndpointKind::ImagesEdits => Err(GatewayError::bad_request(
            "Gemini official API does not support image edits on this compatibility surface yet.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_edit_endpoint")),
        _ => Err(GatewayError::bad_request(
            "Gemini official API adapters currently support only /v1/images/generations, /v1/music/generations, and /v1/videos/generations for media endpoints.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_media_endpoint")),
    }
}

pub async fn execute_official_image(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let upstream_model = surface::resolve_official_image_model(model)?;
    if surface::requested_output_count(req) > 1 {
        return Err(GatewayError::bad_request(
            "Gemini official image generation currently supports only n=1 requests.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_image_count"));
    }
    let prompt = prompt_from_media_request(
        req,
        "Gemini official image generation requires a prompt.",
        "missing_gemini_official_image_prompt",
    )?;
    let request_body = surface::build_image_request_body(req, upstream_model);
    let request_url = format!(
        "{}/models/{}:generateContent",
        payload.base_url.trim_end_matches('/'),
        upstream_model
    );
    let body = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let image = surface::extract_inline_image_from_generate_content_response(&body)?;
    surface::build_openai_images_response_from_bytes(req, &prompt, &[image])
}

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

pub async fn execute_official_video(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    request_url_override: Option<&str>,
) -> Result<Value, GatewayError> {
    let upstream_model = surface::resolve_official_video_model(model)?;
    if surface::requested_output_count(req) > 1 {
        return Err(GatewayError::bad_request(
            "Gemini official video generation currently supports only n=1 requests.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("unsupported_gemini_official_video_count"));
    }
    let prompt = prompt_from_media_request(
        req,
        "Gemini official video generation requires a prompt.",
        "missing_gemini_official_video_prompt",
    )?;
    let request_url = request_url_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}/models/{}:predictLongRunning",
                payload.base_url.trim_end_matches('/'),
                upstream_model
            )
        });
    let request_body = json!({
        "instances": [{
            "prompt": prompt
        }],
        "parameters": {
            "aspectRatio": surface::video_aspect_ratio_from_request(req)
        }
    });
    let operation = send_official_json(
        http,
        payload,
        &request_url,
        &request_body,
        timeout.max(Duration::from_secs(30)),
        extra_headers,
    )
    .await?;
    let operation_name = operation
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video generation did not return an operation name.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_video_missing_operation")
        })?;
    let operation_url =
        if operation_name.starts_with("http://") || operation_name.starts_with("https://") {
            operation_name.to_string()
        } else {
            format!(
                "{}/{}",
                payload.base_url.trim_end_matches('/'),
                operation_name.trim_start_matches('/')
            )
        };

    let deadline = std::time::Instant::now() + timeout.max(Duration::from_secs(600));
    let mut poll = operation;
    while std::time::Instant::now() < deadline {
        if poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
            break;
        }
        sleep(Duration::from_secs(5)).await;
        poll = send_official_get_json(
            http,
            payload,
            &operation_url,
            timeout.max(Duration::from_secs(30)),
            extra_headers,
        )
        .await?;
    }
    if !poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
        return Err(GatewayError::service_unavailable(
            "Gemini official video generation timed out before the operation completed.",
        )
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_video_operation_timeout"));
    }
    if let Some(error) = poll.get("error") {
        return Err(classify_upstream_error(
            502,
            &error.to_string(),
            Some(payload.adapter.as_str()),
        ));
    }
    let video = poll
        .pointer("/response/generateVideoResponse/generatedSamples/0/video")
        .or_else(|| poll.pointer("/response/generatedVideos/0/video"))
        .or_else(|| poll.pointer("/response/generated_videos/0/video"))
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video operation completed without a downloadable video payload.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_no_video_asset")
        })?;
    let video_uri = video
        .get("uri")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini official video operation completed without video.uri.",
            )
            .with_provider(payload.adapter.as_str())
            .with_code("gemini_official_no_video_asset")
        })?;
    let (bytes, content_type) = send_official_get_bytes(
        http,
        payload,
        video_uri,
        timeout.max(Duration::from_secs(120)),
        extra_headers,
    )
    .await?;
    let mime_type = content_type.unwrap_or_else(|| "video/mp4".to_string());
    let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let asset = legacy::GeminiCanvasMediaAsset {
        kind: "video".to_string(),
        url: video_uri.to_string(),
        mime_type,
        download_token: None,
        body_base64: Some(body_base64),
        alt: Some(prompt.clone()),
        width: None,
        height: None,
        duration_seconds: None,
    };
    Ok(surface::build_video_generation_response(
        model, &prompt, &asset, None,
    ))
}

fn build_official_headers(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
) -> HeaderMap {
    let mut api_payload = payload.clone();
    api_payload.adapter = GEMINI_API_LEGACY_ADAPTER.to_string();
    build_upstream_headers_with(&api_payload, extra_headers)
}

async fn send_official_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    body: &Value,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let response = http
        .request(Method::POST, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some(payload.adapter.as_str()),
        ));
    }
    serde_json::from_str::<Value>(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini official API returned invalid JSON: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_invalid_json")
    })
}

async fn send_official_get_json(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<Value, GatewayError> {
    let response = http
        .request(Method::GET, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body_text,
            Some(payload.adapter.as_str()),
        ));
    }
    serde_json::from_str::<Value>(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini official API returned invalid JSON: {error}"
        ))
        .with_provider(payload.adapter.as_str())
        .with_code("gemini_official_invalid_json")
    })
}

async fn send_official_get_bytes(
    http: &Client,
    payload: &ProviderAccountPayload,
    url: &str,
    timeout: Duration,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
    let response = http
        .request(Method::GET, url)
        .headers(build_official_headers(payload, extra_headers))
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = response
        .bytes()
        .await
        .map_err(|error| classify_network_error(&error, Some(payload.adapter.as_str())))?;
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &String::from_utf8_lossy(&bytes),
            Some(payload.adapter.as_str()),
        ));
    }
    Ok((bytes, content_type))
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

fn prompt_from_media_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    if let Some(prompt) = req
        .raw_body
        .get("prompt")
        .or_else(|| req.raw_body.get("input"))
        .or_else(|| req.raw_body.get("lyrics"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(prompt.to_string());
    }
    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(missing_message).with_code(missing_code));
    }
    Ok(prompt)
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

fn compact_response_preview(body_text: &str, max_chars: usize) -> String {
    let normalized = body_text
        .chars()
        .filter(|value| !value.is_control() || matches!(*value, '\n' | '\r' | '\t'))
        .collect::<String>()
        .trim()
        .replace('\r', " ")
        .replace('\n', " ");
    if normalized.len() <= max_chars {
        normalized
    } else {
        format!("{}...", &normalized[..max_chars])
    }
}

fn consume_music_server_content(
    value: &Value,
    audio_mime_type: &mut String,
    audio_bytes: &mut Vec<u8>,
) -> Result<bool, GatewayError> {
    let Some(server_content) = value
        .get("serverContent")
        .or_else(|| value.get("server_content"))
    else {
        return Ok(false);
    };
    let mut saw_audio = false;
    if let Some(chunks) = server_content
        .get("audioChunks")
        .or_else(|| server_content.get("audio_chunks"))
        .and_then(Value::as_array)
    {
        for chunk in chunks {
            let Some(raw_base64) = chunk.get("data").and_then(Value::as_str) else {
                continue;
            };
            let mime_type = chunk
                .get("mimeType")
                .or_else(|| chunk.get("mime_type"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("audio/l16;rate=48000;channels=2");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw_base64.as_bytes())
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Gemini official music websocket returned invalid audio base64: {error}"
                    ))
                    .with_provider(GEMINI_API_LEGACY_ADAPTER)
                    .with_code("gemini_official_music_ws_invalid_audio")
                })?;
            if !bytes.is_empty() {
                *audio_mime_type = mime_type.to_string();
                audio_bytes.extend_from_slice(&bytes);
                saw_audio = true;
            }
        }
        if saw_audio {
            return Ok(true);
        }
    }

    let Some(parts) = server_content
        .get("modelTurn")
        .or_else(|| server_content.get("model_turn"))
        .and_then(|turn| turn.get("parts"))
        .and_then(Value::as_array)
    else {
        return Ok(false);
    };
    for part in parts {
        let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) else {
            continue;
        };
        let Some(raw_base64) = inline_data.get("data").and_then(Value::as_str) else {
            continue;
        };
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("audio/l16;rate=48000;channels=2");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw_base64.as_bytes())
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "Gemini official music websocket returned invalid audio base64: {error}"
                ))
                .with_provider(GEMINI_API_LEGACY_ADAPTER)
                .with_code("gemini_official_music_ws_invalid_audio")
            })?;
        if !bytes.is_empty() {
            *audio_mime_type = mime_type.to_string();
            audio_bytes.extend_from_slice(&bytes);
            saw_audio = true;
        }
    }
    Ok(saw_audio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn consume_music_server_content_accepts_audio_chunks_shape() {
        let mut mime_type = String::from("audio/l16;rate=48000;channels=2");
        let mut audio_bytes = Vec::new();
        let payload = json!({
            "serverContent": {
                "audioChunks": [
                    {
                        "mimeType": "audio/pcm",
                        "data": "AQIDBA=="
                    }
                ]
            }
        });

        let saw_audio =
            consume_music_server_content(&payload, &mut mime_type, &mut audio_bytes).unwrap();

        assert!(saw_audio);
        assert_eq!(mime_type, "audio/pcm");
        assert_eq!(audio_bytes, vec![1, 2, 3, 4]);
    }
}
