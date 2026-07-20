use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use rquest::{Client, Method};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::gemini::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;

use super::{bootstrap_app, build_headers, PROVIDER};

pub type GeminiWebStream = Pin<Box<dyn Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>;

pub async fn execute(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let headers = build_headers(payload, extra_headers, model);
    let bootstrap = bootstrap_app(http, timeout, payload, headers.clone()).await?;
    let request = surface::pack_request(req, model, &bootstrap)?;
    let url = target_url(payload);

    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .query(&request.query)
        .timeout(timeout.max(Duration::from_secs(120)))
        .form(&request.form)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
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

    surface::accumulate_gemini_web_response(&body_text, model)
        .map_err(|error| error.with_provider(PROVIDER))
}

pub async fn execute_stream(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<GeminiWebStream, GatewayError> {
    let headers = build_headers(payload, extra_headers, model);
    let bootstrap = bootstrap_app(http, timeout, payload, headers.clone()).await?;
    let request = surface::pack_request(req, model, &bootstrap)?;
    let url = target_url(payload);

    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .query(&request.query)
        .timeout(timeout.max(Duration::from_secs(120)))
        .form(&request.form)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
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

    let translated = surface::translate_gemini_web_to_openai_sse(&body_text, model)
        .map_err(|error| error.with_provider(PROVIDER))?;
    Ok(Box::pin(futures::stream::iter(
        translated.into_iter().map(Ok),
    )))
}

fn target_url(payload: &ProviderAccountPayload) -> String {
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or(surface::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH);
    format!("{}{}", payload.base_url.trim_end_matches('/'), path)
}
