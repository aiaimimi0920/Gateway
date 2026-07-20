use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_cookie_header, apply_gemini_canvas_navigation_headers,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::summarize_gateway_error;
use crate::upstream::response_types::BinaryUpstreamResponse;
use rquest::{header::HeaderMap, Method};
use std::time::Duration;
use tracing::debug;

pub async fn execute_direct_http_tts_followups(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    model: &str,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    batchexecute_header_id: Option<&str>,
    locator: &gemini_canvas::GeminiCanvasStreamGenerateLocator,
    timeout: Duration,
) -> Result<(String, String), GatewayError> {
    let provider = "gemini_canvas_compatible";
    let trigger_model_header =
        gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);
    let followup_model_header = gemini_canvas::build_text_batchexecute_model_header(
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID),
        batchexecute_header_id,
    );
    let trigger_request = gemini_canvas::build_tts_trigger_request(
        &locator.response_id,
        bootstrap,
        &locator.app_path,
    )?;
    let trigger_body = client
        .send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &trigger_request,
            session,
            &trigger_model_header,
            timeout,
        )
        .await
        .map_err(|error| {
            let mut wrapped = GatewayError::service_unavailable(format!(
                "Gemini Canvas pure HTTP TTS PCck7e trigger failed. app_path={}; header={trigger_model_header}; upstream={}",
                locator.app_path,
                summarize_gateway_error(&error)
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_tts_trigger_failed");
            wrapped.http_status = error.http_status;
            wrapped
        })?;

    let followup_request =
        gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, &locator.app_path)?;
    let followup_body = client
        .send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &followup_request,
            session,
            &followup_model_header,
            timeout,
        )
        .await
        .map_err(|error| {
            let mut wrapped = GatewayError::service_unavailable(format!(
                "Gemini Canvas pure HTTP TTS aPya6c follow-up failed. app_path={}; header={followup_model_header}; upstream={}",
                locator.app_path,
                summarize_gateway_error(&error)
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_tts_followup_failed");
            wrapped.http_status = error.http_status;
            wrapped
        })?;
    Ok((trigger_body, followup_body))
}

pub async fn execute_direct_http_tts_export(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    model: &str,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    batchexecute_header_id: Option<&str>,
    source_path: &str,
    response_text: &str,
    timeout: Duration,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let export_model_header = gemini_canvas::build_text_batchexecute_model_header(
        Some(gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID),
        batchexecute_header_id,
    );
    let export_locale = gemini_canvas::infer_tts_export_locale(
        response_text,
        &gemini_canvas::locale_from_payload(payload),
    );
    let export_request = gemini_canvas::build_tts_audio_export_request(
        response_text,
        &export_locale,
        bootstrap,
        source_path,
    )?;
    client
        .send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &export_request,
            session,
            &export_model_header,
            timeout,
        )
        .await
        .map_err(|error| {
            let mut wrapped = GatewayError::service_unavailable(format!(
                "Gemini Canvas pure HTTP TTS XqA3Ic export failed. app_path={source_path}; locale={export_locale}; header={export_model_header}; upstream={}",
                summarize_gateway_error(&error)
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_tts_export_failed");
            wrapped.http_status = error.http_status;
            wrapped
        })
}

pub async fn resolve_direct_http_tts_audio_response(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    timeout: Duration,
    stream_body: &str,
    trigger_body: &str,
    followup_body: &str,
    export_body: &str,
) -> Result<Option<BinaryUpstreamResponse>, GatewayError> {
    if let Ok(audio) = gemini_canvas::extract_audio_from_tts_export_response(export_body) {
        return Ok(Some(build_binary_audio_response(req, &audio)?));
    }

    let Some(audio_url) = [stream_body, trigger_body, followup_body, export_body]
        .into_iter()
        .find_map(extract_direct_http_tts_audio_url)
    else {
        return Ok(None);
    };

    let response =
        fetch_direct_http_tts_audio(client, payload, req, session, &audio_url, timeout).await?;
    Ok(Some(response))
}

fn build_binary_audio_response(
    req: &CanonicalRelayRequest,
    audio: &gemini_canvas::GeminiCanvasAudio,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    let (body, content_type) = gemini_canvas::build_audio_binary_response(req, audio)?;
    Ok(BinaryUpstreamResponse {
        body: bytes::Bytes::from(body),
        content_type: Some(content_type),
        extra_headers: Vec::new(),
    })
}

async fn fetch_direct_http_tts_audio(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    audio_url: &str,
    timeout: Duration,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let mut headers = HeaderMap::new();
    apply_gemini_canvas_navigation_headers(&mut headers);
    insert_header_map_value(&mut headers, "accept", "audio/*,*/*;q=0.9");
    insert_header_map_value(
        &mut headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    let should_forward_cookies = url::Url::parse(audio_url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .map(|host| {
            let host = host.to_ascii_lowercase();
            host == "gemini.google.com" || host.ends_with(".google.com")
        })
        .unwrap_or(false);
    if should_forward_cookies {
        apply_gemini_canvas_cookie_header(&mut headers, session);
    }

    debug!(
        provider,
        url = %audio_url,
        should_forward_cookies,
        "fetching gemini canvas pure HTTP TTS audio asset"
    );
    let response = client
        .http
        .request(Method::GET, audio_url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(120)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_type_is_html = content_type
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"));
    if !(200..300).contains(&status) || content_type_is_html {
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let upstream =
            classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body_text);
        let mut wrapped = GatewayError::service_unavailable(format!(
            "Gemini Canvas pure HTTP TTS audio download failed. url={audio_url}; upstream={}",
            summarize_gateway_error(&upstream)
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_tts_audio_fetch_failed");
        wrapped.http_status = upstream.http_status;
        return Err(wrapped);
    }

    let body = response
        .bytes()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let audio = gemini_canvas::GeminiCanvasAudio {
        mime_type: select_direct_http_tts_audio_mime_type(content_type.as_deref(), audio_url),
        bytes: body.to_vec(),
    };
    build_binary_audio_response(req, &audio)
}

fn extract_direct_http_tts_audio_url(body: &str) -> Option<String> {
    let normalized = body
        .replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003f", "?")
        .replace("\\u0025", "%")
        .replace("\\/", "/");
    let mut search_from = 0usize;
    while let Some(offset) = normalized[search_from..].find("https://") {
        let start = search_from + offset;
        let rest = &normalized[start..];
        let end = rest
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | ')' | ']' | '}')
            })
            .unwrap_or(rest.len());
        let candidate = rest[..end]
            .trim_end_matches(|c: char| c == ',' || c == ';')
            .trim_end_matches('.');
        let lowered = candidate.to_ascii_lowercase();
        let looks_like_audio = lowered.contains("googlevideo.com")
            || lowered.contains("gvt1.com")
            || lowered.contains(".wav")
            || lowered.contains(".mp3")
            || lowered.contains(".ogg")
            || lowered.contains("filename=")
            || lowered.contains("audio/");
        if looks_like_audio {
            return Some(candidate.to_string());
        }
        search_from = start + "https://".len();
    }
    None
}

fn select_direct_http_tts_audio_mime_type(content_type: Option<&str>, audio_url: &str) -> String {
    if let Some(value) = content_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
    {
        return value;
    }
    let lowered = audio_url.to_ascii_lowercase();
    if lowered.contains(".mp3") {
        "audio/mpeg".to_string()
    } else if lowered.contains(".ogg") {
        "audio/ogg".to_string()
    } else {
        "audio/wav".to_string()
    }
}

pub fn gemini_canvas_tts_direct_http_audio_unavailable_error(
    app_path: &str,
    stream_head: &str,
    stream_tail: &str,
    trigger: &str,
    followup: &str,
    export: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas pure HTTP TTS completed StreamGenerate + captured PCck7e/aPya6c/XqA3Ic follow-ups, but the current direct HTTP path still did not expose a usable audio asset. app_path={app_path}; stream_head={stream_head}; stream_tail={stream_tail}; trigger={trigger}; followup={followup}; export={export}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_tts_direct_http_audio_unavailable")
}

#[cfg(test)]
mod tests {
    use super::{
        extract_direct_http_tts_audio_url, gemini_canvas_tts_direct_http_audio_unavailable_error,
        select_direct_http_tts_audio_mime_type,
    };

    #[test]
    fn extract_direct_http_tts_audio_url_prefers_audio_like_link_and_unescapes() {
        let body = r#"noise https://example.com/not-audio more \"https:\/\/rr3---sn.googlevideo.com\/videoplayback?mime=audio%2Fwav\u0026filename=test.wav\", trailing"#;
        let url = extract_direct_http_tts_audio_url(body).expect("audio url");
        assert_eq!(
            url,
            "https://rr3---sn.googlevideo.com/videoplayback?mime=audio%2Fwav&filename=test.wav"
        );
    }

    #[test]
    fn extract_direct_http_tts_audio_url_trims_suffix_punctuation() {
        let body = "prefix https://storage.googleapis.com/test/final-track.mp3, suffix https://example.com/page";
        let url = extract_direct_http_tts_audio_url(body).expect("audio url");
        assert_eq!(url, "https://storage.googleapis.com/test/final-track.mp3");
    }

    #[test]
    fn select_direct_http_tts_audio_mime_type_prefers_header_then_url_then_default() {
        assert_eq!(
            select_direct_http_tts_audio_mime_type(
                Some("audio/mpeg; charset=utf-8"),
                "https://example.com/file.wav",
            ),
            "audio/mpeg"
        );
        assert_eq!(
            select_direct_http_tts_audio_mime_type(Some("   "), "https://example.com/file.ogg"),
            "audio/ogg"
        );
        assert_eq!(
            select_direct_http_tts_audio_mime_type(None, "https://example.com/file.unknown"),
            "audio/wav"
        );
    }

    #[test]
    fn gemini_canvas_tts_direct_http_audio_unavailable_error_matches_contract() {
        let error = gemini_canvas_tts_direct_http_audio_unavailable_error(
            "/app/tts",
            "stream-head",
            "stream-tail",
            "trigger",
            "followup",
            "export",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_tts_direct_http_audio_unavailable")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas pure HTTP TTS completed StreamGenerate + captured PCck7e/aPya6c/XqA3Ic follow-ups, but the current direct HTTP path still did not expose a usable audio asset. app_path=/app/tts; stream_head=stream-head; stream_tail=stream-tail; trigger=trigger; followup=followup; export=export"
        );
    }
}
