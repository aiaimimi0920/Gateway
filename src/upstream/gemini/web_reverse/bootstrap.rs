use std::time::Duration;

use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::gemini::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;

use super::PROVIDER;

pub async fn bootstrap_app(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    headers: HeaderMap,
) -> Result<surface::GeminiWebBootstrap, GatewayError> {
    let app_url = format!(
        "{}{}",
        payload.base_url.trim_end_matches('/'),
        surface::GEMINI_WEB_DEFAULT_APP_PATH
    );
    let response = http
        .request(Method::GET, &app_url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(30)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let final_url = response.url().to_string();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || surface::response_indicates_session_invalid(status, content_type.as_deref(), &body_text)
    {
        return Err(surface::classify_gemini_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
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
    Ok(merged)
}
