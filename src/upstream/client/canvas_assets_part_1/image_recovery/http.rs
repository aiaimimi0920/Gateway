//! Fixed same-origin reads with request-local Cookie rotation, never persistent writes or generation.
use super::*;
use crate::protocol::gemini_canvas::image_recovery::{
    image_read_error, IMAGE_READ_LIMIT, IMAGE_READ_PATH,
};
use crate::upstream::gemini_canvas_request_headers::apply_gemini_canvas_browserish_text_headers;
use futures::StreamExt;

pub(super) fn read_headers(
    payload: &ProviderAccountPayload,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
) -> Result<HeaderMap, GatewayError> {
    let mut headers = gemini_web_reverse_modular::build_headers(payload, None, "gemini-web-chat");
    apply_gemini_canvas_browserish_text_headers(&mut headers);
    apply_gemini_canvas_cookie_header(&mut headers, session);
    headers.remove("host");
    headers.remove("content-length");
    headers.remove("x-origin");
    for key in [
        gemini_web::GEMINI_WEB_MODEL_HEADER_KEY,
        gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY,
        gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
        gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
    ] {
        headers.remove(key);
    }
    let app = target(payload, "/app")?;
    insert_header_map_value(&mut headers, "origin", &app.origin().ascii_serialization());
    insert_header_map_value(&mut headers, "referer", app.as_str());
    insert_header_map_value(
        &mut headers,
        "content-type",
        "application/x-www-form-urlencoded;charset=UTF-8",
    );
    insert_header_map_value(&mut headers, "x-goog-authuser", &session.auth_user);
    Ok(headers)
}

fn target(payload: &ProviderAccountPayload, path: &str) -> Result<url::Url, GatewayError> {
    let mut base = url::Url::parse(&payload.base_url)
        .map_err(|_| image_read_error("gemini_web_image_read_target_invalid"))?;
    if !matches!(base.scheme(), "http" | "https")
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !matches!(base.path(), "" | "/")
    {
        return Err(image_read_error("gemini_web_image_read_target_invalid"));
    }
    base.set_path(path);
    Ok(base)
}

async fn checked_body(response: rquest::Response) -> Result<String, GatewayError> {
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(image_read_error("gemini_web_image_read_http_failed"));
    }
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if response
        .content_length()
        .is_some_and(|length| length > IMAGE_READ_LIMIT as u64)
    {
        return Err(image_read_error("gemini_web_image_read_too_large"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| image_read_error("gemini_web_image_read_transport_failed"))?;
        if bytes.len().saturating_add(chunk.len()) > IMAGE_READ_LIMIT {
            return Err(image_read_error("gemini_web_image_read_too_large"));
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| image_read_error("gemini_web_image_read_too_large"))?;
        bytes.extend_from_slice(&chunk);
    }
    let body =
        String::from_utf8(bytes).map_err(|_| image_read_error("gemini_web_image_read_invalid"))?;
    if gemini_web::response_indicates_browser_challenge(status, content_type.as_deref(), &body)
        || gemini_web::response_indicates_session_invalid(status, content_type.as_deref(), &body)
    {
        return Err(image_read_error("gemini_web_image_read_session_invalid"));
    }
    Ok(body)
}

pub(super) async fn bootstrap(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    headers: &HeaderMap,
    cookies: &mut SessionCookies,
) -> Result<gemini_web::GeminiWebBootstrap, GatewayError> {
    let target = target(payload, "/app")?;
    let mut headers = headers.clone();
    cookies
        .apply(target.as_str(), &mut headers)
        .map_err(|_| image_read_error("gemini_web_image_read_cookie_invalid"))?;
    let response = client
        .http
        .get(target.as_str())
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .send()
        .await
        .map_err(|_| image_read_error("gemini_web_image_read_transport_failed"))?;
    let response_headers = response.headers().clone();
    let body = checked_body(response).await?;
    let bootstrap = gemini_web::parse_bootstrap_from_app_html(&body, None)
        .map_err(|_| image_read_error("gemini_web_image_read_bootstrap_invalid"))?;
    if [
        &bootstrap.access_token,
        &bootstrap.build_label,
        &bootstrap.session_id,
    ]
    .iter()
    .any(|value| value.as_deref().is_none_or(|value| value.trim().is_empty()))
    {
        return Err(image_read_error("gemini_web_image_read_bootstrap_invalid"));
    }
    // Invalid/partial bootstrap bodies must not advance this recovery's authentication state.
    cookies.receive(target.as_str(), &response_headers);
    Ok(bootstrap)
}

pub(super) async fn history(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    headers: &HeaderMap,
    request: &gemini_web::GeminiWebRequest,
    cookies: &mut SessionCookies,
) -> Result<String, GatewayError> {
    let target = target(payload, IMAGE_READ_PATH)?;
    let mut headers = headers.clone();
    cookies
        .apply(target.as_str(), &mut headers)
        .map_err(|_| image_read_error("gemini_web_image_read_cookie_invalid"))?;
    let response = client
        .http
        .post(target.as_str())
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .query(&request.query)
        .form(&request.form)
        .send()
        .await
        .map_err(|_| image_read_error("gemini_web_image_read_transport_failed"))?;
    let response_headers = response.headers().clone();
    let body = checked_body(response).await?;
    // Only a valid pending candidate can cause another read; parsing errors exit the caller.
    cookies.receive(target.as_str(), &response_headers);
    Ok(body)
}
