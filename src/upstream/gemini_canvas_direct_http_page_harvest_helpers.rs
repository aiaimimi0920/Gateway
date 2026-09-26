use super::contract_helpers::redact_gemini_canvas_url_for_logs;
use super::error_helpers::{
    gemini_canvas_page_harvest_exhausted_error, gemini_canvas_page_harvest_redirect_loop_error,
    gemini_canvas_page_harvest_redirect_missing_location_error,
    gemini_canvas_page_harvest_unsafe_redirect_error,
};
use crate::error::{classify_network_error, GatewayError};
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_asset_helpers::{
    gemini_canvas_page_url_is_allowed, resolve_same_origin_http_redirect,
};
use crate::upstream::gemini_canvas_client_types::GeminiCanvasPageHarvestMode;
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_cookie_header, apply_gemini_canvas_lightweight_navigation_headers,
    apply_gemini_canvas_navigation_headers, apply_gemini_canvas_response_cookies,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_summary, summarize_gateway_error,
};
use rquest::header::HeaderMap;
use rquest::{Client, Method};
pub(crate) async fn fetch_gemini_canvas_direct_http_page_html_once_refreshing_session(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    page_url: &str,
    timeout: std::time::Duration,
    mode: GeminiCanvasPageHarvestMode,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let accept_language = locale_override
        .map(str::to_string)
        .unwrap_or_else(|| gemini_canvas::locale_from_payload(payload));
    let client = if mode.use_plain_http() {
        plain_http
    } else {
        http
    };
    if !gemini_canvas_page_url_is_allowed(page_url) {
        return Err(gemini_canvas_page_harvest_unsafe_redirect_error(400));
    }
    let cookie_count = session
        .cookie_header
        .split(';')
        .filter(|value| !value.trim().is_empty())
        .count();
    let cookie_probe = format!(
        "mode={}, include_cookie={}, include_locale={}, cookie_count={}, cookie_header_len={}, has_1psid={}, has_1psidts={}, has_sapisid={}",
        mode.label(),
        mode.include_cookie_header(),
        mode.include_locale_header(),
        cookie_count,
        session.cookie_header.len(),
        session.cookie_header.contains("__Secure-1PSID="),
        session.cookie_header.contains("__Secure-1PSIDTS="),
        session.cookie_header.contains("SAPISID=")
    );
    let mut current_url = page_url.to_string();
    let mut redirect_trace = Vec::new();

    for _hop in 0..10 {
        let mut headers = HeaderMap::new();
        match mode {
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated => {
                apply_gemini_canvas_navigation_headers(&mut headers);
            }
            GeminiCanvasPageHarvestMode::AnonymousLightweightPlain
            | GeminiCanvasPageHarvestMode::AnonymousLightweightEmulated
            | GeminiCanvasPageHarvestMode::SessionLightweightPlain
            | GeminiCanvasPageHarvestMode::SessionLightweightEmulated => {
                apply_gemini_canvas_lightweight_navigation_headers(&mut headers);
            }
        }
        if mode.include_cookie_header() {
            apply_gemini_canvas_cookie_header(&mut headers, session);
        }
        if mode.include_locale_header() {
            insert_header_map_value(&mut headers, "accept-language", &accept_language);
        }

        let response = client
            .request(Method::GET, &current_url)
            .headers(headers)
            .timeout(timeout.max(std::time::Duration::from_secs(30)))
            .redirect(rquest::redirect::Policy::none())
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(rquest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let set_cookie_count = response
            .headers()
            .get_all(rquest::header::SET_COOKIE)
            .iter()
            .count();

        if response.status().is_redirection() {
            redirect_trace.push(format!(
                "{} -> {} ({status})",
                redact_gemini_canvas_url_for_logs(&current_url),
                location
                    .as_deref()
                    .map(redact_gemini_canvas_url_for_logs)
                    .unwrap_or_else(|| "<none>".to_string())
            ));
            let Some(location_value) = location.as_deref() else {
                let error = gemini_canvas_page_harvest_redirect_missing_location_error(status);
                return Err(append_gateway_error_summary(
                    error,
                    "page_fetch_meta",
                    Some(&format!(
                        "{cookie_probe}, final_url={}, location=<none>, set_cookie_count={}, redirect_trace={}",
                        redact_gemini_canvas_url_for_logs(&current_url),
                        set_cookie_count,
                        redirect_trace.join(" | ")
                    )),
                ));
            };
            let Some(next_url) = resolve_same_origin_http_redirect(&current_url, location_value)
            else {
                let error = gemini_canvas_page_harvest_unsafe_redirect_error(status);
                return Err(append_gateway_error_summary(
                    error,
                    "page_fetch_meta",
                    Some(&format!(
                        "{cookie_probe}, final_url={}, location={}, set_cookie_count={}, redirect_trace={}",
                        redact_gemini_canvas_url_for_logs(&current_url),
                        redact_gemini_canvas_url_for_logs(location_value),
                        set_cookie_count,
                        redirect_trace.join(" | ")
                    )),
                ));
            };
            apply_gemini_canvas_response_cookies(response.headers(), session);
            current_url = next_url;
            continue;
        }

        apply_gemini_canvas_response_cookies(response.headers(), session);
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
            let error =
                classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body_text);
            return Err(append_gateway_error_summary(
                error,
                "page_fetch_meta",
                Some(&format!(
                    "{cookie_probe}, final_url={}, location={}, set_cookie_count={}, content_type={}, redirect_trace={}",
                    redact_gemini_canvas_url_for_logs(&current_url),
                    location
                        .as_deref()
                        .map(redact_gemini_canvas_url_for_logs)
                        .unwrap_or_else(|| "<none>".to_string()),
                    set_cookie_count,
                    content_type.as_deref().unwrap_or("<none>"),
                    redirect_trace.join(" | ")
                )),
            ));
        }

        return Ok(body_text);
    }

    Err(append_gateway_error_summary(
        gemini_canvas_page_harvest_redirect_loop_error(),
        "page_fetch_meta",
        Some(&format!(
            "{cookie_probe}, final_url={}, redirect_trace={}",
            redact_gemini_canvas_url_for_logs(&current_url),
            redirect_trace.join(" | ")
        )),
    ))
}

pub(crate) async fn fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    page_url: &str,
    timeout: std::time::Duration,
    locale_override: Option<&str>,
) -> Result<String, GatewayError> {
    let modes = if locale_override.is_some() {
        vec![
            GeminiCanvasPageHarvestMode::SessionLightweightPlain,
            GeminiCanvasPageHarvestMode::SessionLightweightEmulated,
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated,
            GeminiCanvasPageHarvestMode::AnonymousLightweightPlain,
            GeminiCanvasPageHarvestMode::AnonymousLightweightEmulated,
        ]
    } else {
        vec![
            GeminiCanvasPageHarvestMode::AnonymousLightweightPlain,
            GeminiCanvasPageHarvestMode::AnonymousLightweightEmulated,
            GeminiCanvasPageHarvestMode::SessionLightweightPlain,
            GeminiCanvasPageHarvestMode::SessionLightweightEmulated,
            GeminiCanvasPageHarvestMode::SessionBrowserNavigationEmulated,
        ]
    };
    let mut failures = Vec::new();
    let mut last_error = None;

    for mode in modes {
        match fetch_gemini_canvas_direct_http_page_html_once_refreshing_session(
            http,
            plain_http,
            payload,
            session,
            page_url,
            timeout,
            mode,
            locale_override,
        )
        .await
        {
            Ok(body) => return Ok(body),
            Err(error) => {
                failures.push(format!(
                    "{}: {}",
                    mode.label(),
                    summarize_gateway_error(&error)
                ));
                last_error = Some(error);
            }
        }
    }

    let error = last_error.unwrap_or_else(gemini_canvas_page_harvest_exhausted_error);
    Err(append_gateway_error_summary(
        error,
        "page_fetch_attempts",
        Some(&failures.join(" | ")),
    ))
}
