use super::error_helpers::gemini_canvas_media_bootstrap_exhausted_error;
use crate::error::GatewayError;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasDirectHttpApiKeyTransport;
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_gemini_canvas_browserish_text_headers, apply_gemini_canvas_text_session_headers,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::append_gateway_error_summary;
use crate::upstream::gemini_canvas_runtime_helpers::origin_from_url;
use crate::upstream::header_map_helpers::header_map_string;
use crate::upstream::response_preview_helpers::compact_response_preview;
use rquest::header::HeaderMap;
use serde_json::{json, Value};
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
        "final_url={}, location={}, content_type={}, body_preview={}",
        redact_gemini_canvas_url_for_logs(final_url),
        location
            .map(redact_gemini_canvas_url_for_logs)
            .unwrap_or_else(|| "<none>".to_string()),
        content_type.unwrap_or("<none>"),
        compact_response_preview(body_text, 240)
    )
}

pub(crate) fn build_gemini_canvas_direct_http_text_bootstrap_request_contract(
    bootstrap_url: &str,
    headers: &HeaderMap,
) -> String {
    format!(
        "bootstrap_url={}, is_text_mode=true, cookie={}, origin={}, referer={}",
        redact_gemini_canvas_url_for_logs(bootstrap_url),
        if headers.contains_key("cookie") {
            "<present>"
        } else {
            "<none>"
        },
        redact_gemini_canvas_header_url_for_logs(headers, "origin"),
        redact_gemini_canvas_header_url_for_logs(headers, "referer"),
    )
}

pub(crate) fn build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
    selected_url: &str,
    attempted_urls: &[String],
    session_target_url: &str,
    cookie_header_len: usize,
) -> String {
    format!(
        "bootstrap_source=page_harvest_helper, selected_url={}, attempted_urls={}, is_text_mode=false, session_target_url={}, cookie_header_len={cookie_header_len}",
        redact_gemini_canvas_url_for_logs(selected_url),
        redact_gemini_canvas_urls_for_logs(attempted_urls),
        redact_gemini_canvas_url_for_logs(session_target_url),
    )
}

pub(crate) fn build_gemini_canvas_direct_http_page_harvest_failure_request_contract(
    attempted_urls: &[String],
    session_target_url: &str,
    failures: &[String],
) -> String {
    format!(
        "bootstrap_source=page_harvest_helper, attempted_urls={}, is_text_mode=false, session_target_url={}, failures={}",
        redact_gemini_canvas_urls_for_logs(attempted_urls),
        redact_gemini_canvas_url_for_logs(session_target_url),
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
    let _ = api_key;
    "<redacted>".to_string()
}

pub(crate) fn redact_gemini_canvas_url_for_logs(value: &str) -> String {
    let trimmed = value.trim();
    if matches!(trimmed, "<none>" | "<empty>") {
        return trimmed.to_string();
    }
    if let Ok(mut parsed) = url::Url::parse(trimmed) {
        if !matches!(parsed.scheme(), "http" | "https") {
            return "<redacted-url>".to_string();
        }
        let had_explicit_path = trimmed
            .split_once("://")
            .map(|(_, remainder)| {
                remainder
                    .split(['?', '#'])
                    .next()
                    .is_some_and(|authority_and_path| authority_and_path.contains('/'))
            })
            .unwrap_or(true);
        let _ = parsed.set_password(None);
        let _ = parsed.set_username("");
        parsed.set_query(None);
        parsed.set_fragment(None);
        let mut redacted: String = parsed.into();
        if !had_explicit_path && redacted.ends_with('/') {
            redacted.pop();
        }
        return redacted;
    }
    if trimmed.starts_with('/') {
        return trimmed
            .split(['?', '#'])
            .next()
            .unwrap_or("<redacted-url>")
            .to_string();
    }
    "<redacted-url>".to_string()
}

pub(crate) fn redact_gemini_canvas_urls_for_logs(values: &[String]) -> String {
    values
        .iter()
        .map(|value| redact_gemini_canvas_url_for_logs(value))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn redact_gemini_canvas_header_url_for_logs(headers: &HeaderMap, name: &str) -> String {
    header_map_string(headers, name)
        .map(|value| redact_gemini_canvas_url_for_logs(&value))
        .unwrap_or_else(|| "<none>".to_string())
}

pub(crate) fn sensitive_header_presence_for_logs(headers: &HeaderMap, name: &str) -> &'static str {
    if headers.contains_key(name) {
        "<present>"
    } else {
        "<none>"
    }
}
