use futures::StreamExt;
use rquest::header::HeaderMap;
use rquest::{Client, Method};
use std::time::Duration;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::gemini_web;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel;
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_cookie_header, apply_gemini_canvas_response_cookies,
    apply_gemini_canvas_signaler_headers,
};
use crate::{protocol::gemini_canvas, routing::candidate::ProviderAccountPayload};

pub(crate) async fn send_gemini_canvas_signaler_request_refreshing_session_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    method: Method,
    url: &str,
    content_type: Option<&str>,
    webchannel_content_type: Option<&str>,
    body: Option<String>,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let origin = payload.base_url.trim_end_matches('/');
    let referer = format!("{}/", payload.base_url.trim_end_matches('/'));
    let accept_language = locale_override
        .map(str::to_string)
        .unwrap_or_else(|| gemini_canvas::locale_from_payload(payload));
    let mut headers = HeaderMap::new();
    apply_gemini_canvas_signaler_headers(&mut headers);
    apply_gemini_canvas_cookie_header(&mut headers, session);
    insert_header_map_value(&mut headers, "accept-language", &accept_language);
    insert_header_map_value(&mut headers, "origin", origin);
    insert_header_map_value(&mut headers, "referer", &referer);
    insert_header_map_value(
        &mut headers,
        "x-goog-authuser",
        &gemini_canvas::direct_http_auth_user(payload),
    );
    if let Some(content_type) = content_type {
        insert_header_map_value(&mut headers, "content-type", content_type);
    }
    if let Some(webchannel_content_type) = webchannel_content_type {
        insert_header_map_value(
            &mut headers,
            "x-webchannel-content-type",
            webchannel_content_type,
        );
    }
    let mut request = http.request(method, url).headers(headers).timeout(timeout);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    apply_gemini_canvas_response_cookies(response.headers(), session);
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    if !(200..300).contains(&status)
        || gemini_web::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || gemini_web::response_indicates_session_invalid(
            status,
            content_type.as_deref(),
            &body_text,
        )
    {
        return Err(classify_gemini_canvas_pure_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    Ok(body_text)
}

pub(crate) async fn send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    url: &str,
    timeout: Duration,
    aid_hint: u64,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let origin = payload.base_url.trim_end_matches('/');
    let referer = format!("{}/", payload.base_url.trim_end_matches('/'));
    let accept_language = locale_override
        .map(str::to_string)
        .unwrap_or_else(|| gemini_canvas::locale_from_payload(payload));
    let mut headers = HeaderMap::new();
    apply_gemini_canvas_signaler_headers(&mut headers);
    apply_gemini_canvas_cookie_header(&mut headers, session);
    insert_header_map_value(&mut headers, "accept-language", &accept_language);
    insert_header_map_value(&mut headers, "origin", origin);
    insert_header_map_value(&mut headers, "referer", &referer);
    insert_header_map_value(
        &mut headers,
        "x-goog-authuser",
        &gemini_canvas::direct_http_auth_user(payload),
    );
    let response = http
        .request(Method::GET, url)
        .headers(headers)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    apply_gemini_canvas_response_cookies(response.headers(), session);
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_gemini_canvas_signaler_body(response, provider, aid_hint).await?;
    if !(200..300).contains(&status)
        || gemini_web::response_indicates_browser_challenge(
            status,
            content_type.as_deref(),
            &body_text,
        )
        || gemini_web::response_indicates_session_invalid(
            status,
            content_type.as_deref(),
            &body_text,
        )
    {
        return Err(classify_gemini_canvas_pure_http_error(
            status,
            content_type.as_deref(),
            &body_text,
        ));
    }
    Ok(body_text)
}

pub(crate) async fn refresh_gemini_canvas_image_edit_signaler_creds_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    refresh_token: &str,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let url = format!(
        "https://signaler-pa.clients6.google.com/punctual/v1/refreshCreds?key={}&gsessionid={}",
        channel.api_key, channel.gsession_id
    );
    let body = gemini_canvas::build_image_edit_signaler_refresh_creds_body(refresh_token);
    send_gemini_canvas_signaler_request_refreshing_session_with_http(
        http,
        payload,
        session,
        Method::POST,
        &url,
        Some("application/json+protobuf"),
        None,
        Some(body),
        timeout,
        locale_override,
    )
    .await
}

pub(crate) async fn refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
    http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    body: &str,
    timeout: Duration,
    locale_override: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    let Some(refresh_token) = gemini_canvas::extract_signaler_long_poll_refresh_token(body) else {
        return Ok(None);
    };
    refresh_gemini_canvas_image_edit_signaler_creds_with_http(
        http,
        payload,
        session,
        channel,
        &refresh_token,
        timeout,
        locale_override,
    )
    .await?;
    Ok(Some(refresh_token))
}

async fn collect_gemini_canvas_signaler_body(
    response: rquest::Response,
    provider: &str,
    aid_hint: u64,
) -> Result<String, GatewayError> {
    let mut stream = response.bytes_stream();
    let mut body_text = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                body_text.push_str(String::from_utf8_lossy(&chunk).as_ref());
                if gemini_canvas::extract_page_blob_media_assets(
                    &body_text,
                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                )
                .is_ok()
                {
                    return Ok(body_text);
                }
                if !gemini_canvas::extract_signaler_app_paths(&body_text).is_empty() {
                    return Ok(body_text);
                }
                if aid_hint == 0
                    && gemini_canvas::extract_signaler_long_poll_refresh_token(&body_text).is_some()
                    && gemini_canvas::extract_signaler_long_poll_max_aid(&body_text).is_some()
                {
                    return Ok(body_text);
                }
            }
            Err(error) => {
                if !body_text.is_empty() {
                    return Ok(body_text);
                }
                return Err(classify_network_error(&error, Some(provider)));
            }
        }
    }

    Ok(body_text)
}
