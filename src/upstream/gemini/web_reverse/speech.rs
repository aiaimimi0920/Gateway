//! One literal-input export. Optional HTTP checkpoint only; no browser or business replay.
use futures::StreamExt;
use rquest::{Client, Response};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};

use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayRequest, gemini::web_reverse as surface},
    routing::candidate::ProviderAccountPayload,
    upstream::response_types::BinaryUpstreamResponse,
};
use surface::speech::{
    invalid_speech_response, speech_export_request, validate_speech, without_speech_replay,
    MAX_SPEECH_WIRE_BYTES, SPEECH_RPC_PATH,
};

async fn body(response: Response, limit: usize) -> Result<String, GatewayError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(invalid_speech_response());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| transport_error())?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(invalid_speech_response());
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| invalid_speech_response())?;
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| invalid_speech_response())
}

fn transport_error() -> GatewayError {
    without_speech_replay(GatewayError::service_unavailable("Gemini Web speech transport failed; no automatic retry or browser fallback was attempted.")
        .with_provider(super::PROVIDER).with_code("gemini_web_speech_transport_failed"))
}

async fn checked_body(response: Response, limit: usize) -> Result<String, GatewayError> {
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body = body(response, limit).await?;
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(status, content_type.as_deref(), &body)
        || surface::response_indicates_session_invalid(status, content_type.as_deref(), &body)
    {
        // Keep safe challenge/auth classification without logging private response text or codes.
        let marker = if surface::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body,
        ) {
            "<html>challenge</html>"
        } else if surface::response_indicates_session_invalid(
            status,
            content_type.as_deref(),
            &body,
        ) {
            "<html>sign in</html>"
        } else {
            ""
        };
        let mut error = surface::classify_gemini_web_http_error(status, None, marker);
        error.message =
            "Gemini Web speech HTTP request failed; private response content was omitted.".into();
        return Err(without_speech_replay(error));
    }
    Ok(body)
}

pub async fn execute_speech(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    // Public dispatch prepares call_flow before this owner, and all options precede any I/O.
    let input = validate_speech(req, model)?;
    if !crate::protocol::gemini_canvas::pure_http_enabled(payload) {
        return Err(without_speech_replay(
            GatewayError::bad_request(
                "Gemini Web speech requires HTTP execution; browser fallback is not supported.",
            )
            .with_provider(super::PROVIDER)
            .with_code("gemini_web_speech_http_required"),
        ));
    }
    let mut headers = super::build_headers(payload, extra_headers, "gemini-web-chat");
    let base = payload.base_url.trim_end_matches('/');
    let app_url = format!("{base}/app");
    let mut cookies = super::http_session::HttpSession::start(&app_url, payload, &headers)
        .await
        .map_err(without_speech_replay)?;
    cookies
        .apply(&app_url, &mut headers)
        .map_err(without_speech_replay)?;
    let response = http
        .get(&app_url)
        .headers(headers.clone())
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout)
        .send()
        .await
        .map_err(|_| transport_error())?;
    let response_headers = response.headers().clone();
    let html = match checked_body(response, 8 * 1024 * 1024).await {
        Ok(html) => html,
        Err(error) => {
            cookies.invalidate(&error).await;
            return Err(error);
        }
    };
    let language = payload
        .extra_body
        .as_ref()
        .and_then(|e| e.get("language"))
        .and_then(Value::as_str);
    // A fresh successful bootstrap is mandatory; never revive stale cache after an invalid page.
    let bootstrap = surface::parse_bootstrap_from_app_html(&html, language)
        .map_err(|_| invalid_speech_response())?;
    if [
        &bootstrap.access_token,
        &bootstrap.build_label,
        &bootstrap.session_id,
    ]
    .iter()
    .any(|value| value.as_deref().is_none_or(|value| value.trim().is_empty()))
    {
        return Err(invalid_speech_response());
    }
    let request = speech_export_request(&input, &bootstrap)?;
    let target = format!("{base}{SPEECH_RPC_PATH}");
    cookies.receive(&app_url, &response_headers);
    cookies
        .apply(&target, &mut headers)
        .map_err(without_speech_replay)?;
    let response = http
        .post(&target)
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout)
        .query(&request.query)
        .form(&request.form)
        .send()
        .await
        .map_err(|_| transport_error())?;
    let response_headers = response.headers().clone();
    let body = match checked_body(response, MAX_SPEECH_WIRE_BYTES).await {
        Ok(body) => body,
        Err(error) => {
            cookies.invalidate(&error).await;
            return Err(error);
        }
    };
    let (bytes, mime) = surface::speech_response::speech_audio(&body, input.format)?;
    cookies.receive(&target, &response_headers);
    cookies.commit().await;
    Ok(BinaryUpstreamResponse {
        body: bytes.into(),
        content_type: Some(mime.into()),
        extra_headers: vec![],
    })
}
