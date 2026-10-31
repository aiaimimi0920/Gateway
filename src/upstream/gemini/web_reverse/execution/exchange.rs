//! Preparation completes before any I/O; upload URLs are never caller-configurable.
use std::collections::HashMap;
use std::time::Duration;

use rquest::{Client, Method};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini::web_reverse as surface;
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::web_reverse::{
    bootstrap::bootstrap_with_cookies, build_headers, http_session::HttpSession, uploads, PROVIDER,
};

pub(super) async fn exchange(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    upload_url: &str,
) -> Result<(String, HttpSession), GatewayError> {
    let (sanitized, files) = surface::prepare_media(req)?;
    surface::validate_text_request(&sanitized)?;
    let mut headers = build_headers(payload, extra_headers, model);
    let (bootstrap, mut cookies) =
        bootstrap_with_cookies(http, timeout, payload, headers.clone()).await?;
    let references = uploads::upload_files(http, timeout, &bootstrap, files, upload_url).await?;
    let request = surface::pack_request_with_uploaded_files(&sanitized, &bootstrap, &references)?;
    let path = payload
        .chat_completions_path
        .as_deref()
        .unwrap_or(surface::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH);
    let target = format!("{}{}", payload.base_url.trim_end_matches('/'), path);
    cookies.apply(&target, &mut headers)?;
    let response = http
        .request(Method::POST, &target)
        .headers(headers)
        .query(&request.query)
        .redirect(rquest::redirect::Policy::none())
        .timeout(timeout.max(Duration::from_secs(120)))
        .form(&request.form)
        .send()
        .await
        .map_err(|e| classify_network_error(&e, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let response_headers = response.headers().clone();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body =
        collect_bounded_upstream_text_with_provider(response, "Gemini Web response", PROVIDER)
            .await?;
    if !(200..300).contains(&status)
        || surface::response_indicates_browser_challenge(status, content_type.as_deref(), &body)
        || surface::response_indicates_session_invalid(status, content_type.as_deref(), &body)
    {
        let error = response_error(req, status, content_type.as_deref(), &body);
        cookies.invalidate(&error).await;
        return Err(error);
    }
    cookies.receive(&target, &response_headers);
    Ok((body, cookies))
}

pub(super) fn response_error(
    req: &CanonicalRelayRequest,
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    if !surface::has_media(req) {
        return surface::classify_gemini_web_http_error(status, content_type, body);
    }
    // Preserve observed challenge/auth classification while never copying private
    // content or arbitrary upstream code/message fields into the error projection.
    let marker = if surface::response_indicates_browser_challenge(status, content_type, body) {
        "<html>challenge</html>"
    } else if surface::response_indicates_session_invalid(status, content_type, body) {
        "<html>sign in</html>"
    } else {
        ""
    };
    let mut error = surface::classify_gemini_web_http_error(status, None, marker);
    error.message = "Gemini Web media request failed; private response content was omitted.".into();
    error
}
