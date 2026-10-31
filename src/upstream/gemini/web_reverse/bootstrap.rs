use std::time::Duration;

use futures::StreamExt;
use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::gemini::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;

use super::http_session::HttpSession;
use super::PROVIDER;

async fn app_body(response: rquest::Response) -> Result<String, GatewayError> {
    let invalid = || {
        GatewayError::server_error("Gemini Web bootstrap response is invalid or exceeds limits.")
            .with_provider(PROVIDER)
            .with_code("gemini_web_bootstrap_body_invalid")
    };
    const LIMIT: usize = 8 * 1024 * 1024;
    if response.content_length().is_some_and(|n| n > LIMIT as u64) {
        return Err(invalid());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| classify_network_error(&e, Some(PROVIDER)))?;
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(invalid());
        }
        bytes.try_reserve(chunk.len()).map_err(|_| invalid())?;
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| invalid())
}

pub async fn bootstrap_app(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    headers: HeaderMap,
) -> Result<surface::GeminiWebBootstrap, GatewayError> {
    bootstrap_with_cookies(http, timeout, payload, headers)
        .await
        .map(|(bootstrap, _)| bootstrap)
}

pub(super) async fn bootstrap_with_cookies(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    mut headers: HeaderMap,
) -> Result<(surface::GeminiWebBootstrap, HttpSession), GatewayError> {
    let app_url = format!(
        "{}{}",
        payload.base_url.trim_end_matches('/'),
        surface::GEMINI_WEB_DEFAULT_APP_PATH
    );
    let mut cookies = HttpSession::start(&app_url, payload, &headers).await?;
    cookies.apply(&app_url, &mut headers)?;
    let response = http
        .request(Method::GET, &app_url)
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout.max(Duration::from_secs(30)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let final_url = response.uri().to_string();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let response_headers = response.headers().clone();
    let body_text = app_body(response).await?;
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || surface::response_indicates_session_invalid(status, content_type.as_deref(), &body_text)
    {
        let error =
            surface::classify_gemini_web_http_error(status, content_type.as_deref(), &body_text);
        cookies.invalidate(&error).await;
        return Err(error);
    }
    let fallback = super::bootstrap_from_payload_cache(payload.extra_body.as_ref());
    let bootstrap = surface::parse_bootstrap_from_app_html(
        &body_text,
        payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get("language").and_then(Value::as_str)),
    )
    .or_else(|primary_error| fallback.clone().ok_or(primary_error))?;
    let mut merged = surface::merge_bootstrap_from_fallback(bootstrap, fallback.as_ref());
    if merged.app_page_path.is_none() {
        merged.app_page_path = surface::extract_app_page_path_from_url(&final_url);
    }
    cookies.receive(&final_url, &response_headers);
    Ok((merged, cookies))
}
