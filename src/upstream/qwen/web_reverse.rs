use std::collections::HashMap;
use std::time::Duration;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::{Client, Method};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::qwen::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::headers::build_upstream_headers_with;

const PROVIDER: &str = "qwen_web_compatible";

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Qwen Web reverse adapters use create-chat + chat replay and are not supported by the generic request planner.",
    )
    .with_code("unsupported_qwen_web_request_plan")
}

pub async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let headers = build_qwen_web_headers(payload, extra_headers);
    let chat_id = create_qwen_web_chat(
        &client.http,
        client.timeout,
        payload,
        model,
        headers.clone(),
    )
    .await?;
    let body = surface::pack_qwen_web(req, model, &chat_id)?;
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or(surface::QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH);
    let url = format!("{}{}", payload.base_url.trim_end_matches('/'), path);

    let response = client
        .http
        .request(Method::POST, &url)
        .headers(headers)
        .query(&[("chat_id", chat_id.as_str())])
        .timeout(client.timeout.max(Duration::from_secs(120)))
        .json(&body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !response.status().is_success()
        || surface::response_indicates_browser_challenge(status, content_type.as_deref(), "")
    {
        let body_text = response.text().await.unwrap_or_default();
        return Err(surface::classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    if content_type
        .as_deref()
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
    {
        let body_text = response.text().await.unwrap_or_default();
        return Err(surface::classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }

    surface::accumulate_qwen_web_stream(response, model)
        .await
        .map_err(|error| error.with_provider(PROVIDER))
}

pub async fn execute_stream(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
    GatewayError,
> {
    let headers = build_qwen_web_headers(payload, extra_headers);
    let chat_id = create_qwen_web_chat(
        &client.http,
        client.timeout,
        payload,
        model,
        headers.clone(),
    )
    .await?;
    let body = surface::pack_qwen_web(req, model, &chat_id)?;
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or(surface::QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH);
    let url = format!("{}{}", payload.base_url.trim_end_matches('/'), path);

    let response = client
        .http
        .request(Method::POST, &url)
        .headers(headers)
        .query(&[("chat_id", chat_id.as_str())])
        .timeout(client.timeout.max(Duration::from_secs(120)))
        .json(&body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !response.status().is_success()
        || content_type
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
    {
        let body_text = response.text().await.unwrap_or_default();
        return Err(surface::classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }

    Ok(Box::pin(surface::translate_qwen_web_stream(
        response.bytes_stream(),
        model.to_string(),
    )))
}

async fn create_qwen_web_chat(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    model: &str,
    headers: HeaderMap,
) -> Result<String, GatewayError> {
    let url = format!(
        "{}{}",
        payload.base_url.trim_end_matches('/'),
        surface::QWEN_WEB_DEFAULT_CREATE_CHAT_PATH
    );
    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(30)))
        .json(&surface::pack_create_chat(model))
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
        || content_type
            .as_deref()
            .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"))
    {
        return Err(surface::classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    let body: serde_json::Value = serde_json::from_str(&body_text).map_err(|error| {
        surface::classify_qwen_web_http_error(
            status,
            content_type.as_deref(),
            &format!("{body_text}\n\nQwen Web JSON decode failure: {error}"),
        )
    })?;
    body.get("data")
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            GatewayError::server_error("Qwen Web create-chat response did not include data.id.")
                .with_provider(PROVIDER)
                .with_code("qwen_web_missing_chat_id")
        })
}

fn build_qwen_web_headers(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
) -> HeaderMap {
    let mut headers = build_upstream_headers_with(payload, extra_headers);
    let origin = payload.base_url.trim_end_matches('/').to_string();
    if !headers.contains_key("origin") {
        if let Ok(value) = HeaderValue::from_str(&origin) {
            headers.insert(HeaderName::from_static("origin"), value);
        }
    }
    if !headers.contains_key("referer") {
        let referer = format!("{origin}/c/guest");
        if let Ok(value) = HeaderValue::from_str(&referer) {
            headers.insert(HeaderName::from_static("referer"), value);
        }
    }
    headers
}
