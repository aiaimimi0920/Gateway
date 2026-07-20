use crate::error::{classify_network_error, GatewayError};
use crate::object_storage::gateway_object_storage;
use crate::protocol::gemini_canvas;
use crate::protocol::gemini_web;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;
use crate::upstream::gemini_canvas_asset_helpers::resolve_relative_url;
use crate::upstream::gemini_canvas_client_types::{
    GeminiCanvasDirectHttpApiKeyTransport, GeminiCanvasPageHarvestMode,
};
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_browser_fetch_client_hints, apply_gemini_canvas_browserish_text_headers,
    apply_gemini_canvas_cookie_header, apply_gemini_canvas_lightweight_navigation_headers,
    apply_gemini_canvas_navigation_headers, apply_gemini_canvas_page_context_headers,
    apply_gemini_canvas_response_cookies, apply_gemini_canvas_signed_headers,
    apply_gemini_canvas_text_session_headers,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_summary, summarize_gateway_error,
};
use crate::upstream::gemini_canvas_runtime_helpers::{
    current_unix_timestamp_i64, gemini_canvas_http_origin, origin_from_url,
};
use crate::upstream::header_map_helpers::header_map_string;
use crate::upstream::response_preview_helpers::compact_response_preview;
use rquest::header::{HeaderMap, HeaderValue};
use rquest::{Client, Method};
use serde_json::{json, Value};
use tracing::debug;

pub(crate) const GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS: &[&str] = &[
    "AIzaSyCqyCcs2R2e7AegGjvFAwG98wlamtbHvZY",
    "AIzaSyD6n9asBjvx1yBHfhFhfw_kpS9Faq0BZHM",
    "AIzaSyAPW83vB9zFQqfpMup_cMJdELqDQkWvTho",
    "AIzaSyBWW50ghQ5qHpMg1gxHV7U9t0wHE0qIUk4",
    "AIzaSyDmUQ6sj3nbs_ZiSsxsbP7L6qlPDT3cr4Q",
    "AIzaSyAHCfkEDYwQD6HuUx2DyX3VylTrKZG7doM",
];

pub(crate) fn gemini_canvas_direct_http_bootstrap_candidates(
    payload: &ProviderAccountPayload,
    base_url: &str,
    share_id: &str,
    is_image_mode: bool,
) -> Vec<String> {
    let app_bootstrap_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    let share_bootstrap_url = gemini_canvas::direct_http_referrer(base_url, share_id);
    let mut candidates = Vec::new();

    if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
        if let Some(program_page_url) =
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload, base_url,
            )
        {
            candidates.push(program_page_url);
        }
    }

    let ordered_defaults = if is_image_mode {
        [app_bootstrap_url, share_bootstrap_url]
    } else {
        [share_bootstrap_url, app_bootstrap_url]
    };
    for candidate in ordered_defaults {
        if !candidates.iter().any(|existing| existing == &candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

pub(crate) fn gemini_canvas_direct_http_api_key_transports(
    request_url: &str,
) -> &'static [GeminiCanvasDirectHttpApiKeyTransport] {
    let request_origin = origin_from_url(request_url).unwrap_or_default();
    if request_origin.contains("clients6.google.com") {
        &[
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
        ]
    } else {
        &[
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
            GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
        ]
    }
}

pub(crate) fn build_gemini_canvas_direct_http_bootstrap_response_meta(
    final_url: &str,
    location: Option<&str>,
    content_type: Option<&str>,
    body_text: &str,
) -> String {
    format!(
        "final_url={final_url}, location={}, content_type={}, body_preview={}",
        location.unwrap_or("<none>"),
        content_type.unwrap_or("<none>"),
        compact_response_preview(body_text, 240)
    )
}

pub(crate) fn build_gemini_canvas_direct_http_text_bootstrap_request_contract(
    bootstrap_url: &str,
    headers: &HeaderMap,
) -> String {
    format!(
        "bootstrap_url={bootstrap_url}, is_text_mode=true, cookie={}, origin={}, referer={}",
        if headers.contains_key("cookie") {
            "<present>"
        } else {
            "<none>"
        },
        header_map_string(headers, "origin").unwrap_or_else(|| "<none>".to_string()),
        header_map_string(headers, "referer").unwrap_or_else(|| "<none>".to_string()),
    )
}

pub(crate) fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
    selected_url: &str,
    attempted_urls: &[String],
    session_target_url: &str,
    cookie_header_len: usize,
) -> String {
    format!(
        "bootstrap_source=page_harvest_helper, selected_url={selected_url}, attempted_urls={}, is_text_mode=false, session_target_url={session_target_url}, cookie_header_len={cookie_header_len}",
        attempted_urls.join(",")
    )
}

pub(crate) fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
    attempted_urls: &[String],
    session_target_url: &str,
    failures: &[String],
) -> String {
    format!(
        "bootstrap_source=page_harvest_helper, attempted_urls={}, is_text_mode=false, session_target_url={session_target_url}, failures={}",
        attempted_urls.join(","),
        failures.join(" | ")
    )
}

pub(crate) fn build_gemini_canvas_direct_http_page_harvest_failure_error(
    last_error: Option<GatewayError>,
    attempted_urls: &[String],
    session_target_url: &str,
    failures: &[String],
) -> GatewayError {
    append_gateway_error_summary(
        last_error.unwrap_or_else(gemini_canvas_media_bootstrap_exhausted_error),
        "bootstrap_request_contract",
        Some(
            &build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
                attempted_urls,
                session_target_url,
                failures,
            ),
        ),
    )
}

pub(crate) fn build_gemini_canvas_direct_http_text_bootstrap_failure_error(
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
    bootstrap_request_contract: &str,
    bootstrap_response_meta: &str,
) -> GatewayError {
    append_gateway_error_summary(
        append_gateway_error_summary(
            classify_gemini_canvas_pure_http_error(status, content_type, body_text),
            "bootstrap_request_contract",
            Some(bootstrap_request_contract),
        ),
        "bootstrap_response_meta",
        Some(bootstrap_response_meta),
    )
}

pub(crate) fn resolve_gemini_canvas_direct_http_bootstrap_page_path(
    bootstrap_page_url: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
) -> String {
    gemini_web::extract_app_page_path_from_url(bootstrap_page_url)
        .or_else(|| bootstrap.app_page_path.clone())
        .unwrap_or_else(|| gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string())
}

pub(crate) fn resolve_gemini_canvas_direct_http_referer(
    effective_base_url: &str,
    page_path: &str,
    text_preflight_source_path: &str,
    is_text_mode: bool,
    is_image_mode: bool,
) -> String {
    if is_text_mode || is_image_mode {
        if is_text_mode
            && gemini_canvas_program_web_reverse_modular::is_concrete_gemini_canvas_app_path(
                text_preflight_source_path,
            )
        {
            format!("{effective_base_url}{text_preflight_source_path}")
        } else {
            format!("{effective_base_url}/")
        }
    } else {
        format!("{effective_base_url}{page_path}")
    }
}

pub(crate) fn resolve_gemini_canvas_direct_http_preflight_source_path(
    payload: &ProviderAccountPayload,
) -> String {
    gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_source_path(payload)
        .unwrap_or_else(|| gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string())
}

pub(crate) fn resolve_gemini_canvas_direct_http_stream_generate_model_header<'a>(
    harvested_header: Option<&'a str>,
    is_text_mode: bool,
    is_image_mode: bool,
) -> &'a str {
    if let Some(harvested) = harvested_header {
        harvested
    } else if is_text_mode {
        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER
    } else if is_image_mode {
        gemini_canvas::GEMINI_CANVAS_IMAGE_STREAM_GENERATE_MODEL_HEADER
    } else {
        gemini_canvas::GEMINI_CANVAS_MEDIA_STREAM_GENERATE_MODEL_HEADER
    }
}

pub(crate) fn gemini_canvas_direct_http_text_state_variant_preflight_specs(
) -> [(usize, usize, Value, &'static str); 5] {
    [
        (41usize, 40usize, Value::from(0), "side_nav_open_by_default"),
        (87usize, 86usize, Value::from(1), "popup_zs_visits_cooldown"),
        (87usize, 86usize, Value::from(2), "popup_zs_visits_cooldown"),
        (
            94usize,
            93usize,
            Value::String("NULL".to_string()),
            "current_popup_id",
        ),
        (
            94usize,
            93usize,
            Value::String("HUMAN_REVIEWER_DISCLOSURE".to_string()),
            "current_popup_id",
        ),
    ]
}

pub(crate) fn gemini_canvas_direct_http_text_fast_version_preflight_spec(
) -> (usize, usize, Value, &'static str) {
    (
        179usize,
        178usize,
        Value::String("2025-12-16".to_string()),
        "enforce_default_to_fast_version",
    )
}

pub(crate) fn build_gemini_canvas_direct_http_text_generic_preflight_specs(
    language: &str,
) -> Vec<(&'static str, Value, &'static str)> {
    vec![
        (
            "otAQ7b",
            json!([]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "sJBwce",
            json!([[1, 2]]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "DYBcR",
            json!([language]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "GPRiHf",
            json!([]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "maGuAc",
            json!([0]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "maGuAc",
            json!([1]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "Te6DCf",
            json!([[language], [1]]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
        ),
        (
            "mhs1xe",
            json!([[1, 3]]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
        ),
        (
            "K4WWud",
            json!([[0], [language]]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
        ),
        (
            "ku4Jyf",
            json!([
                language,
                null,
                null,
                null,
                4,
                null,
                null,
                [2, 3, 7, 17],
                null,
                []
            ]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
        ),
        (
            "ozz5Z",
            json!([[
                [[null, "1", 447]],
                [[null, "1", 448]],
                [[null, "1", 702]],
                [[null, "1", 961]],
                [[null, "1", 960]],
                [[null, "1", 1062]],
                [[null, "1", 1240]],
                [[null, "1", 1237]],
                [[null, "1", 1238]],
                [[null, "1", 1239]],
                [[null, "1", 1241]]
            ]]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
        ),
        (
            "CNgdBe",
            json!([1, [language], 0]),
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
        ),
    ]
}

pub(crate) fn apply_gemini_canvas_direct_http_stream_generate_headers(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    request_uuid: &str,
    model_header: &str,
    session: Option<&gemini_canvas::GeminiCanvasPureHttpSession>,
    apply_browserish_headers: bool,
) {
    if apply_browserish_headers {
        apply_gemini_canvas_browserish_text_headers(headers);
    }
    if let Some(session) = session {
        apply_gemini_canvas_text_session_headers(headers, session);
    }
    insert_header_map_value(
        headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_KEY,
        model_header,
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY,
        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_2,
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY,
        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_3,
    );
    insert_header_map_value(
        headers,
        gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
        &format!("[\"{request_uuid}\",1]"),
    );
}

pub(crate) fn redact_gemini_canvas_api_key_for_logs(api_key: &str) -> String {
    let trimmed = api_key.trim();
    if trimmed.len() <= 12 {
        return "<redacted>".to_string();
    }
    format!(
        "{}...{}",
        &trimmed[..8],
        &trimmed[trimmed.len().saturating_sub(4)..]
    )
}

pub(crate) fn gemini_canvas_image_json_attempts_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP image replay exhausted all known JSON contracts.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_json_attempts_exhausted")
}

pub(crate) fn gemini_canvas_page_harvest_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest exhausted all request modes.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_exhausted")
}

pub(crate) fn gemini_canvas_page_harvest_redirect_loop_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest exceeded the redirect follow limit.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_redirect_loop")
}

pub(crate) fn gemini_canvas_page_harvest_redirect_missing_location_error(
    status: u16,
) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest returned a redirect without a usable Location header.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_redirect_missing_location");
    error.http_status = Some(status);
    error
}

pub(crate) fn gemini_canvas_media_bootstrap_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas media bootstrap exhausted all page harvest candidates.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_bootstrap_exhausted")
}

pub(crate) fn gemini_canvas_media_fetch_redirect_exhausted_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas direct HTTP media download exhausted redirect/handoff attempts.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_redirect_exhausted")
}

pub(crate) fn gemini_canvas_media_fetch_cookie_mismatch_redirect_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP media download redirected into Google account login/CookieMismatch instead of returning the requested asset.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_cookie_mismatch")
}

pub(crate) fn gemini_canvas_media_fetch_cookie_mismatch_html_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP media download resolved to a Google login/CookieMismatch HTML page instead of a binary asset.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_cookie_mismatch")
}

pub(crate) fn gemini_canvas_media_fetch_bad_redirect_error(next_url: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas direct HTTP media redirect URL was not usable: {next_url}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_bad_redirect")
}

pub(crate) fn gemini_canvas_media_fetch_redirect_missing_location_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas direct HTTP media download returned a redirect without a usable Location header.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_redirect_missing_location")
}

pub(crate) fn gemini_canvas_media_fetch_bad_asset_url_error(asset_url: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas direct HTTP media asset URL was not usable: {asset_url}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_bad_asset_url")
}

pub(crate) fn gemini_canvas_image_fetch_missing_inline_bytes_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP image materialization completed without inline bytes.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_fetch_missing_inline_bytes")
}

pub(crate) fn gemini_canvas_image_fetch_invalid_inline_bytes_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas direct HTTP image materialization returned invalid base64 bytes: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_fetch_invalid_inline_bytes")
}

pub(crate) fn gemini_canvas_pure_http_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas pure HTTP response did not return valid JSON: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_pure_http_invalid_json")
}

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
        apply_gemini_canvas_response_cookies(response.headers(), session);
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
                current_url,
                location.as_deref().unwrap_or("<none>")
            ));
            if let Some(next_url) = location
                .as_deref()
                .and_then(|value| resolve_relative_url(&current_url, value))
            {
                current_url = next_url;
                continue;
            }
            let error = gemini_canvas_page_harvest_redirect_missing_location_error(status);
            return Err(append_gateway_error_summary(
                error,
                "page_fetch_meta",
                Some(&format!(
                    "{cookie_probe}, final_url={current_url}, location={}, set_cookie_count={}, redirect_trace={}",
                    location.as_deref().unwrap_or("<none>"),
                    set_cookie_count,
                    redirect_trace.join(" | ")
                )),
            ));
        }

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
                    "{cookie_probe}, final_url={current_url}, location={}, set_cookie_count={}, content_type={}, redirect_trace={}",
                    location.as_deref().unwrap_or("<none>"),
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
            "{cookie_probe}, final_url={current_url}, redirect_trace={}",
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

pub(crate) async fn send_gemini_canvas_direct_http_json_with_options(
    http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    request_url: &str,
    request_body: &Value,
    timeout: std::time::Duration,
    api_key_override: Option<&str>,
    api_key_transport: GeminiCanvasDirectHttpApiKeyTransport,
    signed_origin_override: Option<&str>,
    referer_override: Option<&str>,
    preserve_cross_origin_origin: bool,
    preserve_cross_origin_referer: bool,
    include_signed_headers: bool,
) -> Result<Value, GatewayError> {
    let provider = "gemini_canvas_compatible";
    let storage_state = gateway_object_storage()?
        .read_json(&runtime.runtime_state_object_key)
        .await?;
    let auth_user = gemini_canvas::direct_http_auth_user(payload);
    let google_api_key = api_key_override
        .map(str::to_string)
        .or_else(|| gemini_canvas::direct_http_google_api_key(payload, &storage_state));
    let session = gemini_canvas::storage_state_to_pure_http_session(
        &storage_state,
        request_url,
        payload.base_url.trim_end_matches('/'),
        &auth_user,
    )?;
    let page_origin = gemini_canvas_http_origin(payload);
    let target_origin = origin_from_url(request_url).unwrap_or_else(|| page_origin.clone());
    let signed_origin = signed_origin_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| target_origin.clone());
    let authorization = gemini_canvas::build_sapisid_authorization(
        &session.sapisid,
        &signed_origin,
        current_unix_timestamp_i64(),
    )?;
    let referer = referer_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            let referer_path =
                gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref())
                    .and_then(|bootstrap| bootstrap.app_page_path)
                    .unwrap_or_else(|| gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string());
            format!("{}{}", payload.base_url.trim_end_matches('/'), referer_path)
        });

    let mut headers = HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        rquest::header::ACCEPT,
        HeaderValue::from_static("application/json"),
    );
    if api_key_transport.include_header() {
        if let Some(api_key) = google_api_key.as_deref() {
            insert_header_map_value(&mut headers, "x-goog-api-key", api_key);
        }
    }
    insert_header_map_value(
        &mut headers,
        "user-agent",
        gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT,
    );
    insert_header_map_value(
        &mut headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    apply_browser_fetch_client_hints(&mut headers, request_url, &page_origin);
    let include_page_context = apply_gemini_canvas_page_context_headers(
        &mut headers,
        &page_origin,
        &referer,
        &target_origin,
        preserve_cross_origin_origin,
    );
    if include_signed_headers {
        apply_gemini_canvas_signed_headers(
            &mut headers,
            &session,
            &signed_origin,
            &referer,
            &authorization,
            true,
        );
    }
    if !include_page_context {
        headers.remove("origin");
        if preserve_cross_origin_referer {
            insert_header_map_value(&mut headers, "referer", &referer);
        } else {
            headers.remove("referer");
        }
    }

    let google_api_key_hint = google_api_key
        .as_deref()
        .map(redact_gemini_canvas_api_key_for_logs);
    let request_contract = format!(
        "url={request_url}, signed_headers={}, include_page_context={}, preserve_cross_origin_origin={}, api_key_transport={}, origin={}, referer={}, x-origin={}, authorization={}, cookie={}, x-goog-api-key={}, query-key={}",
        include_signed_headers,
        include_page_context,
        preserve_cross_origin_origin,
        api_key_transport.label(),
        header_map_string(&headers, "origin").unwrap_or_else(|| "<none>".to_string()),
        header_map_string(&headers, "referer").unwrap_or_else(|| "<none>".to_string()),
        header_map_string(&headers, "x-origin").unwrap_or_else(|| "<none>".to_string()),
        if headers.contains_key("authorization") {
            "<present>"
        } else {
            "<none>"
        },
        if headers.contains_key("cookie") {
            "<present>"
        } else {
            "<none>"
        },
        header_map_string(&headers, "x-goog-api-key").unwrap_or_else(|| "<none>".to_string()),
        if api_key_transport.include_query() && google_api_key.is_some() {
            "<present>"
        } else {
            "<none>"
        }
    );
    debug!(
        provider,
        has_google_api_key = google_api_key.is_some(),
        google_api_key_transport = api_key_transport.label(),
        google_api_key_hint = google_api_key_hint.as_deref().unwrap_or("none"),
        signed_origin = %signed_origin,
        referer = %referer,
        url = %request_url,
        "sending gemini canvas pure HTTP generateContent request"
    );
    let mut request_builder = http
        .request(Method::POST, request_url)
        .headers(headers)
        .timeout(timeout.max(std::time::Duration::from_secs(30)));
    if api_key_transport.include_query() {
        if let Some(api_key) = google_api_key.as_deref() {
            request_builder = request_builder.query(&[("key", api_key)]);
        }
    }
    let response = request_builder
        .json(request_body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let final_url = response.url().to_string();
    let status = response.status().as_u16();
    let location = response
        .headers()
        .get(rquest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let response_meta = format!(
        "final_url={final_url}, location={}, content_type={}, body_preview={}",
        location.as_deref().unwrap_or("<none>"),
        content_type.as_deref().unwrap_or("<none>"),
        compact_response_preview(&body_text, 240)
    );

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
        let mut error =
            classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body_text);
        error.message = format!(
            "{}; request_contract={request_contract}; response_meta={response_meta}",
            error.message
        );
        return Err(error);
    }

    serde_json::from_str::<Value>(&body_text).map_err(|error| {
        gemini_canvas_pure_http_invalid_json_error(provider, error.to_string().as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquest::header::{HeaderMap, HeaderValue};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::future::Future;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        serde_json::from_value(json!({
            "adapter": adapter,
            "baseUrl": base_url,
            "apiKey": "sk-test"
        }))
        .expect("payload")
    }

    #[test]
    fn gemini_canvas_direct_http_bootstrap_candidates_prefer_program_page_for_program_adapter() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        payload.extra_body = Some(HashMap::from([
            (
                "shareId".to_string(),
                Value::String("fe24c455a570".to_string()),
            ),
            (
                "canvasProgramUrl".to_string(),
                Value::String("https://gemini.google.com/app/4abc4e7577b6149f".to_string()),
            ),
        ]));

        let candidates = gemini_canvas_direct_http_bootstrap_candidates(
            &payload,
            "https://gemini.google.com",
            "fe24c455a570",
            false,
        );
        assert_eq!(
            candidates.first().map(String::as_str),
            Some("https://gemini.google.com/app/4abc4e7577b6149f")
        );
        assert!(candidates
            .iter()
            .any(|entry| entry == "https://gemini.google.com/share/fe24c455a570"));
        assert!(candidates
            .iter()
            .any(|entry| entry == "https://gemini.google.com/app"));
    }

    #[test]
    fn gemini_canvas_direct_http_bootstrap_candidates_keep_legacy_order_without_program_page() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let candidates = gemini_canvas_direct_http_bootstrap_candidates(
            &payload,
            "https://gemini.google.com",
            "fe24c455a570",
            false,
        );
        assert_eq!(
            candidates,
            vec![
                "https://gemini.google.com/share/fe24c455a570".to_string(),
                "https://gemini.google.com/app".to_string()
            ]
        );
    }

    #[test]
    fn direct_http_image_json_attempts_exhausted_error_matches_contract() {
        let error = gemini_canvas_image_json_attempts_exhausted_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_json_attempts_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP image replay exhausted all known JSON contracts."
        );
    }

    #[test]
    fn direct_http_page_harvest_exhausted_error_matches_contract() {
        let error = gemini_canvas_page_harvest_exhausted_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_page_harvest_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP page harvest exhausted all request modes."
        );
    }

    #[test]
    fn direct_http_page_harvest_redirect_loop_error_matches_contract() {
        let error = gemini_canvas_page_harvest_redirect_loop_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_page_harvest_redirect_loop")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP page harvest exceeded the redirect follow limit."
        );
    }

    #[test]
    fn direct_http_page_harvest_redirect_missing_location_error_matches_contract() {
        let error = gemini_canvas_page_harvest_redirect_missing_location_error(302);
        assert_eq!(error.http_status, Some(302));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_page_harvest_redirect_missing_location")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP page harvest returned a redirect without a usable Location header."
        );
    }

    #[test]
    fn fetch_gemini_canvas_direct_http_page_html_once_returns_string_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let plain_http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "0".to_string(),
        };

        assert_future_output(
            fetch_gemini_canvas_direct_http_page_html_once_refreshing_session(
                &http,
                &plain_http,
                &payload,
                &mut session,
                "https://gemini.google.com/app",
                std::time::Duration::from_secs(1),
                GeminiCanvasPageHarvestMode::AnonymousLightweightPlain,
                Some("en-US"),
            ),
        );
    }

    #[test]
    fn fetch_gemini_canvas_direct_http_page_html_refreshing_session_returns_string_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let plain_http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "0".to_string(),
        };

        assert_future_output(
            fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                &http,
                &plain_http,
                &payload,
                &mut session,
                "https://gemini.google.com/app",
                std::time::Duration::from_secs(1),
                Some("en-US"),
            ),
        );
    }

    #[test]
    fn send_gemini_canvas_direct_http_json_with_options_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<Value, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let runtime = gemini_canvas::GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/test.json".to_string(),
            share_id: "share-123".to_string(),
            api_base_url: "https://generativelanguage.googleapis.com".to_string(),
        };
        let request_body = json!({
            "contents": [{
                "parts": [{ "text": "generate an image" }]
            }]
        });

        assert_future_output(send_gemini_canvas_direct_http_json_with_options(
            &http,
            &payload,
            &runtime,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini:generateContent",
            &request_body,
            std::time::Duration::from_secs(1),
            Some("AIza-test"),
            GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
            None,
            None,
            false,
            false,
            true,
        ));
    }

    #[test]
    fn build_gemini_canvas_direct_http_bootstrap_response_meta_preserves_shape() {
        let meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
            "https://gemini.google.com/app",
            Some("https://gemini.google.com/share/example"),
            Some("text/html; charset=utf-8"),
            "<html>bootstrap</html>",
        );

        assert_eq!(
            meta,
            "final_url=https://gemini.google.com/app, location=https://gemini.google.com/share/example, content_type=text/html; charset=utf-8, body_preview=<html>bootstrap</html>"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_bootstrap_response_meta_preserves_empty_preview() {
        let meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
            "https://gemini.google.com/app",
            None,
            None,
            "",
        );

        assert_eq!(
            meta,
            "final_url=https://gemini.google.com/app, location=<none>, content_type=<none>, body_preview=<empty>"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_bootstrap_request_contract_preserves_shape() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", HeaderValue::from_static("SID=abc"));
        headers.insert(
            "origin",
            HeaderValue::from_static("https://gemini.google.com"),
        );
        headers.insert(
            "referer",
            HeaderValue::from_static("https://gemini.google.com/share/example"),
        );

        let contract = build_gemini_canvas_direct_http_text_bootstrap_request_contract(
            "https://gemini.google.com/share/example",
            &headers,
        );

        assert_eq!(
            contract,
            "bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie=<present>, origin=https://gemini.google.com, referer=https://gemini.google.com/share/example"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_bootstrap_request_contract_marks_missing_headers() {
        let headers = HeaderMap::new();

        let contract = build_gemini_canvas_direct_http_text_bootstrap_request_contract(
            "https://gemini.google.com/app",
            &headers,
        );

        assert_eq!(
            contract,
            "bootstrap_url=https://gemini.google.com/app, is_text_mode=true, cookie=<none>, origin=<none>, referer=<none>"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract_preserves_shape() {
        let attempted_urls = vec![
            "https://gemini.google.com/share/example".to_string(),
            "https://gemini.google.com/app".to_string(),
        ];

        let contract = build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
            "https://gemini.google.com/share/example",
            &attempted_urls,
            "https://gemini.google.com/share/example",
            128,
        );

        assert_eq!(
            contract,
            "bootstrap_source=page_harvest_helper, selected_url=https://gemini.google.com/share/example, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, cookie_header_len=128"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract_preserves_empty_attempts(
    ) {
        let attempted_urls = Vec::<String>::new();

        let contract = build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
            "https://gemini.google.com/app",
            &attempted_urls,
            "https://gemini.google.com/app",
            0,
        );

        assert_eq!(
            contract,
            "bootstrap_source=page_harvest_helper, selected_url=https://gemini.google.com/app, attempted_urls=, is_text_mode=false, session_target_url=https://gemini.google.com/app, cookie_header_len=0"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract_preserves_shape() {
        let attempted_urls = vec![
            "https://gemini.google.com/share/example".to_string(),
            "https://gemini.google.com/app".to_string(),
        ];
        let failures = vec![
            "https://gemini.google.com/share/example: 503 challenge".to_string(),
            "https://gemini.google.com/app: 401 session_invalid".to_string(),
        ];

        let contract = build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
            &attempted_urls,
            "https://gemini.google.com/share/example",
            &failures,
        );

        assert_eq!(
            contract,
            "bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, failures=https://gemini.google.com/share/example: 503 challenge | https://gemini.google.com/app: 401 session_invalid"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract_preserves_empty_lists()
    {
        let attempted_urls = Vec::<String>::new();
        let failures = Vec::<String>::new();

        let contract = build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
            &attempted_urls,
            "https://gemini.google.com/app",
            &failures,
        );

        assert_eq!(
            contract,
            "bootstrap_source=page_harvest_helper, attempted_urls=, is_text_mode=false, session_target_url=https://gemini.google.com/app, failures="
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_failure_error_preserves_upstream_contract() {
        let attempted_urls = vec![
            "https://gemini.google.com/share/example".to_string(),
            "https://gemini.google.com/app".to_string(),
        ];
        let failures = vec![
            "https://gemini.google.com/share/example: 503 challenge".to_string(),
            "https://gemini.google.com/app: 401 session_invalid".to_string(),
        ];
        let mut upstream = GatewayError::unauthorized("session invalid")
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_session_invalid");
        upstream.http_status = Some(401);

        let error = build_gemini_canvas_direct_http_page_harvest_failure_error(
            Some(upstream),
            &attempted_urls,
            "https://gemini.google.com/share/example",
            &failures,
        );

        assert_eq!(error.http_status, Some(401));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_pure_http_session_invalid")
        );
        assert_eq!(
            error.message,
            "session invalid; bootstrap_request_contract=bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, failures=https://gemini.google.com/share/example: 503 challenge | https://gemini.google.com/app: 401 session_invalid"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_page_harvest_failure_error_falls_back_to_bootstrap_contract()
    {
        let attempted_urls = vec!["https://gemini.google.com/app".to_string()];
        let failures = vec!["https://gemini.google.com/app: 503 exhausted".to_string()];

        let error = build_gemini_canvas_direct_http_page_harvest_failure_error(
            None,
            &attempted_urls,
            "https://gemini.google.com/app",
            &failures,
        );

        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_bootstrap_exhausted")
        );
        assert_eq!(
            error.message,
            "Gemini Canvas media bootstrap exhausted all page harvest candidates.; bootstrap_request_contract=bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/app, failures=https://gemini.google.com/app: 503 exhausted"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_bootstrap_failure_error_preserves_session_invalid_contract(
    ) {
        let error = build_gemini_canvas_direct_http_text_bootstrap_failure_error(
            401,
            Some("text/html; charset=utf-8"),
            "<!DOCTYPE html><a href=\"https://accounts.google.com\">sign in</a>",
            "bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie=<present>, origin=https://gemini.google.com, referer=https://gemini.google.com/share/example",
            "final_url=https://gemini.google.com/share/example, location=<none>, content_type=text/html; charset=utf-8, body_preview=<html>sign in</html>",
        );

        assert_eq!(error.http_status, Some(401));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_pure_http_session_invalid")
        );
        assert_eq!(
            error.message,
            "Gemini Canvas pure HTTP replay session is invalid or expired.; bootstrap_request_contract=bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie=<present>, origin=https://gemini.google.com, referer=https://gemini.google.com/share/example; bootstrap_response_meta=final_url=https://gemini.google.com/share/example, location=<none>, content_type=text/html; charset=utf-8, body_preview=<html>sign in</html>"
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_bootstrap_failure_error_preserves_generic_upstream_contract(
    ) {
        let error = build_gemini_canvas_direct_http_text_bootstrap_failure_error(
            502,
            Some("application/json"),
            "{\"message\":\"temporary upstream failure\"}",
            "bootstrap_url=https://gemini.google.com/app, is_text_mode=true, cookie=<none>, origin=<none>, referer=<none>",
            "final_url=https://gemini.google.com/app, location=<none>, content_type=application/json, body_preview={\"message\":\"temporary upstream failure\"}",
        );

        assert_eq!(error.http_status, Some(502));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.message,
            "temporary upstream failure; bootstrap_request_contract=bootstrap_url=https://gemini.google.com/app, is_text_mode=true, cookie=<none>, origin=<none>, referer=<none>; bootstrap_response_meta=final_url=https://gemini.google.com/app, location=<none>, content_type=application/json, body_preview={\"message\":\"temporary upstream failure\"}"
        );
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_bootstrap_page_path_prefers_url_contract() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: None,
            session_id: None,
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: Some("/app/from-bootstrap".to_string()),
        };

        let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
            "https://gemini.google.com/app/4abc4e7577b6149f",
            &bootstrap,
        );

        assert_eq!(page_path, "/app/4abc4e7577b6149f");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_bootstrap_page_path_falls_back_to_bootstrap_contract() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: None,
            session_id: None,
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: Some("/app/from-bootstrap".to_string()),
        };

        let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
            "https://gemini.google.com/share/example",
            &bootstrap,
        );

        assert_eq!(page_path, "/app/from-bootstrap");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_bootstrap_page_path_defaults_to_app_contract() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: None,
            session_id: None,
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let page_path = resolve_gemini_canvas_direct_http_bootstrap_page_path(
            "https://gemini.google.com/share/example",
            &bootstrap,
        );

        assert_eq!(page_path, gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_referer_prefers_concrete_text_source_contract() {
        let referer = resolve_gemini_canvas_direct_http_referer(
            "https://gemini.google.com",
            "/app/from-bootstrap",
            "/app/from-program",
            true,
            false,
        );

        assert_eq!(referer, "https://gemini.google.com/app/from-program");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_referer_falls_back_to_root_for_image_contract() {
        let referer = resolve_gemini_canvas_direct_http_referer(
            "https://gemini.google.com",
            "/app/from-bootstrap",
            gemini_web::GEMINI_WEB_DEFAULT_APP_PATH,
            false,
            true,
        );

        assert_eq!(referer, "https://gemini.google.com/");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_referer_uses_bootstrap_page_for_media_contract() {
        let referer = resolve_gemini_canvas_direct_http_referer(
            "https://gemini.google.com",
            "/app/from-bootstrap",
            gemini_web::GEMINI_WEB_DEFAULT_APP_PATH,
            false,
            false,
        );

        assert_eq!(referer, "https://gemini.google.com/app/from-bootstrap");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_preflight_source_path_prefers_program_app_path_contract() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        payload.extra_body = Some(HashMap::from([
            (
                "shareId".to_string(),
                Value::String("canvas-share-789".to_string()),
            ),
            (
                "appPath".to_string(),
                Value::String("/app/4abc4e7577b6149f".to_string()),
            ),
            (
                "conversationId".to_string(),
                Value::String("c_ignored_because_app_path_wins".to_string()),
            ),
        ]));

        let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

        assert_eq!(source_path, "/app/4abc4e7577b6149f");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_preflight_source_path_falls_back_to_conversation_contract()
    {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        payload.extra_body = Some(HashMap::from([
            (
                "shareId".to_string(),
                Value::String("canvas-share-789".to_string()),
            ),
            (
                "conversationId".to_string(),
                Value::String("c_4abc4e7577b6149f".to_string()),
            ),
        ]));

        let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

        assert_eq!(source_path, "/app/4abc4e7577b6149f");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_preflight_source_path_defaults_to_app_contract() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");

        let source_path = resolve_gemini_canvas_direct_http_preflight_source_path(&payload);

        assert_eq!(source_path, gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_stream_generate_model_header_prefers_harvested_contract() {
        let header = resolve_gemini_canvas_direct_http_stream_generate_model_header(
            Some("[1,\"HARVESTED\"]"),
            false,
            true,
        );

        assert_eq!(header, "[1,\"HARVESTED\"]");
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_stream_generate_model_header_uses_text_default_contract() {
        let header =
            resolve_gemini_canvas_direct_http_stream_generate_model_header(None, true, false);

        assert_eq!(
            header,
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER
        );
    }

    #[test]
    fn resolve_gemini_canvas_direct_http_stream_generate_model_header_uses_media_default_contract()
    {
        let header =
            resolve_gemini_canvas_direct_http_stream_generate_model_header(None, false, false);

        assert_eq!(
            header,
            gemini_canvas::GEMINI_CANVAS_MEDIA_STREAM_GENERATE_MODEL_HEADER
        );
    }

    #[test]
    fn gemini_canvas_direct_http_text_state_variant_preflight_specs_preserve_marker_order() {
        let specs = gemini_canvas_direct_http_text_state_variant_preflight_specs();

        assert_eq!(
            specs
                .iter()
                .map(|(_, _, _, marker)| *marker)
                .collect::<Vec<_>>(),
            vec![
                "side_nav_open_by_default",
                "popup_zs_visits_cooldown",
                "popup_zs_visits_cooldown",
                "current_popup_id",
                "current_popup_id",
            ]
        );
    }

    #[test]
    fn gemini_canvas_direct_http_text_state_variant_preflight_specs_preserve_tail_values() {
        let specs = gemini_canvas_direct_http_text_state_variant_preflight_specs();

        assert_eq!(
            specs[0],
            (41usize, 40usize, Value::from(0), "side_nav_open_by_default")
        );
        assert_eq!(
            specs[1],
            (87usize, 86usize, Value::from(1), "popup_zs_visits_cooldown",)
        );
        assert_eq!(
            specs[2],
            (87usize, 86usize, Value::from(2), "popup_zs_visits_cooldown",)
        );
        assert_eq!(
            specs[3],
            (
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
            )
        );
        assert_eq!(
            specs[4],
            (
                94usize,
                93usize,
                Value::String("HUMAN_REVIEWER_DISCLOSURE".to_string()),
                "current_popup_id",
            )
        );
    }

    #[test]
    fn gemini_canvas_direct_http_text_fast_version_preflight_spec_preserves_marker_contract() {
        let (state_len, tail_index, tail_value, marker) =
            gemini_canvas_direct_http_text_fast_version_preflight_spec();

        assert_eq!(state_len, 179usize);
        assert_eq!(tail_index, 178usize);
        assert_eq!(tail_value, Value::String("2025-12-16".to_string()));
        assert_eq!(marker, "enforce_default_to_fast_version");
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_generic_preflight_specs_preserve_rpcid_order() {
        let specs = build_gemini_canvas_direct_http_text_generic_preflight_specs("en-US");

        assert_eq!(specs.len(), 12);
        assert_eq!(specs[0].0, "otAQ7b");
        assert_eq!(specs[1].0, "sJBwce");
        assert_eq!(specs[2].0, "DYBcR");
        assert_eq!(specs[11].0, "CNgdBe");
        assert_eq!(
            specs[0].2,
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT
        );
        assert_eq!(
            specs[11].2,
            gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER
        );
    }

    #[test]
    fn build_gemini_canvas_direct_http_text_generic_preflight_specs_embed_language_contract() {
        let specs = build_gemini_canvas_direct_http_text_generic_preflight_specs("zh-CN");

        assert_eq!(specs[2].1, json!(["zh-CN"]));
        assert_eq!(specs[6].1, json!([["zh-CN"], [1]]));
        assert_eq!(specs[8].1, json!([[0], ["zh-CN"]]));
        assert_eq!(specs[11].1, json!([1, ["zh-CN"], 0]));
    }

    #[test]
    fn apply_gemini_canvas_direct_http_stream_generate_headers_preserves_text_contract() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let session = gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "SID=abc; SAPISID=def".to_string(),
            sapisid: "def".to_string(),
            auth_user: "1".to_string(),
        };
        let mut headers = HeaderMap::new();

        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            &payload,
            "request-123",
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER,
            Some(&session),
            true,
        );

        assert_eq!(
            header_map_string(&headers, "accept").as_deref(),
            Some("*/*")
        );
        assert_eq!(
            header_map_string(&headers, "accept-language").as_deref(),
            Some("en-US")
        );
        assert_eq!(
            header_map_string(&headers, "cookie").as_deref(),
            Some("SID=abc; SAPISID=def")
        );
        assert_eq!(
            header_map_string(&headers, "x-goog-authuser").as_deref(),
            Some("1")
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_KEY).as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER)
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_2_KEY).as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_2)
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_3_KEY).as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_3)
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY)
                .as_deref(),
            Some("[\"request-123\",1]")
        );
    }

    #[test]
    fn apply_gemini_canvas_direct_http_stream_generate_headers_skips_browserish_and_session_when_disabled(
    ) {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let mut headers = HeaderMap::new();

        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            &payload,
            "request-456",
            gemini_canvas::GEMINI_CANVAS_IMAGE_STREAM_GENERATE_MODEL_HEADER,
            None,
            false,
        );

        assert_eq!(header_map_string(&headers, "accept"), None);
        assert_eq!(header_map_string(&headers, "cookie"), None);
        assert_eq!(header_map_string(&headers, "x-goog-authuser"), None);
        assert_eq!(
            header_map_string(&headers, "accept-language").as_deref(),
            Some("en-US")
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_MODEL_HEADER_KEY).as_deref(),
            Some(gemini_canvas::GEMINI_CANVAS_IMAGE_STREAM_GENERATE_MODEL_HEADER)
        );
        assert_eq!(
            header_map_string(&headers, gemini_web::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY)
                .as_deref(),
            Some("[\"request-456\",1]")
        );
    }

    #[test]
    fn media_bootstrap_exhausted_error_matches_contract() {
        let error = gemini_canvas_media_bootstrap_exhausted_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_bootstrap_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas media bootstrap exhausted all page harvest candidates."
        );
    }

    #[test]
    fn media_fetch_redirect_exhausted_error_matches_contract() {
        let error = gemini_canvas_media_fetch_redirect_exhausted_error();
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_redirect_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media download exhausted redirect/handoff attempts."
        );
    }

    #[test]
    fn media_fetch_cookie_mismatch_redirect_error_matches_contract() {
        let error = gemini_canvas_media_fetch_cookie_mismatch_redirect_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_cookie_mismatch")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media download redirected into Google account login/CookieMismatch instead of returning the requested asset."
        );
    }

    #[test]
    fn media_fetch_cookie_mismatch_html_error_matches_contract() {
        let error = gemini_canvas_media_fetch_cookie_mismatch_html_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_cookie_mismatch")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media download resolved to a Google login/CookieMismatch HTML page instead of a binary asset."
        );
    }

    #[test]
    fn media_fetch_bad_redirect_error_matches_contract() {
        let error = gemini_canvas_media_fetch_bad_redirect_error("https://bad hop");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_bad_redirect")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media redirect URL was not usable: https://bad hop"
        );
    }

    #[test]
    fn media_fetch_redirect_missing_location_error_matches_contract() {
        let error = gemini_canvas_media_fetch_redirect_missing_location_error();
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_redirect_missing_location")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media download returned a redirect without a usable Location header."
        );
    }

    #[test]
    fn media_fetch_bad_asset_url_error_matches_contract() {
        let error = gemini_canvas_media_fetch_bad_asset_url_error("blob:bad");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_media_fetch_bad_asset_url")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP media asset URL was not usable: blob:bad"
        );
    }

    #[test]
    fn image_fetch_missing_inline_bytes_error_matches_contract() {
        let error = gemini_canvas_image_fetch_missing_inline_bytes_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_fetch_missing_inline_bytes")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP image materialization completed without inline bytes."
        );
    }

    #[test]
    fn image_fetch_invalid_inline_bytes_error_matches_contract() {
        let error = gemini_canvas_image_fetch_invalid_inline_bytes_error("bad base64");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_fetch_invalid_inline_bytes")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas direct HTTP image materialization returned invalid base64 bytes: bad base64"
        );
    }

    #[test]
    fn pure_http_invalid_json_error_matches_contract() {
        let error =
            gemini_canvas_pure_http_invalid_json_error("gemini_canvas_compatible", "bad json");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_pure_http_invalid_json")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas pure HTTP response did not return valid JSON: bad json"
        );
    }

    #[test]
    fn gemini_canvas_direct_http_api_key_transports_prefer_query_for_clients6() {
        let transports = gemini_canvas_direct_http_api_key_transports(
            "https://geminiweb-pa.clients6.google.com/v1beta/models/gemini-2.5-flash-image-preview:generateContent",
        );

        assert_eq!(
            transports,
            &[
                GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
                GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
                GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
            ]
        );
    }

    #[test]
    fn gemini_canvas_direct_http_api_key_transports_prefer_header_for_googleapis() {
        let transports = gemini_canvas_direct_http_api_key_transports(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        );

        assert_eq!(
            transports,
            &[
                GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
                GeminiCanvasDirectHttpApiKeyTransport::QueryOnly,
                GeminiCanvasDirectHttpApiKeyTransport::HeaderAndQuery,
            ]
        );
    }

    #[test]
    fn gemini_canvas_public_page_api_key_fallbacks_keep_candidate_contract() {
        assert_eq!(GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS.len(), 6);
        assert!(GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS
            .iter()
            .all(|candidate| candidate.starts_with("AIza")));
    }

    #[test]
    fn redact_gemini_canvas_api_key_for_logs_omits_secret_body() {
        assert_eq!(
            redact_gemini_canvas_api_key_for_logs("AIzaSyCqyCcs2R2e7AegGjvFAwG98wlamtbHvZY"),
            "AIzaSyCq...HvZY"
        );
    }
}
