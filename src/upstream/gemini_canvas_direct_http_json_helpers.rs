use super::contract_helpers::{
    redact_gemini_canvas_api_key_for_logs, redact_gemini_canvas_header_url_for_logs,
    redact_gemini_canvas_url_for_logs, sensitive_header_presence_for_logs,
};
use super::error_helpers::gemini_canvas_pure_http_invalid_json_error;
use crate::error::{classify_network_error, GatewayError};
use crate::object_storage::gateway_object_storage;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasDirectHttpApiKeyTransport;
use crate::upstream::gemini_canvas_error_helpers::classify_gemini_canvas_pure_http_error;
use crate::upstream::gemini_canvas_request_headers::{
    apply_browser_fetch_client_hints, apply_gemini_canvas_page_context_headers,
    apply_gemini_canvas_signed_headers,
};
use crate::upstream::gemini_canvas_runtime_helpers::{
    current_unix_timestamp_i64, gemini_canvas_http_origin, origin_from_url,
};
use crate::upstream::response_preview_helpers::compact_response_preview;
use rquest::header::{HeaderMap, HeaderValue};
use rquest::{Client, Method};
use serde_json::Value;
use tracing::debug;
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
    let redacted_request_url = redact_gemini_canvas_url_for_logs(request_url);
    let redacted_signed_origin = redact_gemini_canvas_url_for_logs(&signed_origin);
    let redacted_referer = redact_gemini_canvas_url_for_logs(&referer);
    let request_contract = format!(
        "url={redacted_request_url}, signed_headers={}, include_page_context={}, preserve_cross_origin_origin={}, api_key_transport={}, origin={}, referer={}, x-origin={}, authorization={}, cookie={}, x-goog-api-key={}, query-key={}",
        include_signed_headers,
        include_page_context,
        preserve_cross_origin_origin,
        api_key_transport.label(),
        redact_gemini_canvas_header_url_for_logs(&headers, "origin"),
        redact_gemini_canvas_header_url_for_logs(&headers, "referer"),
        redact_gemini_canvas_header_url_for_logs(&headers, "x-origin"),
        sensitive_header_presence_for_logs(&headers, "authorization"),
        sensitive_header_presence_for_logs(&headers, "cookie"),
        sensitive_header_presence_for_logs(&headers, "x-goog-api-key"),
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
        signed_origin = %redacted_signed_origin,
        referer = %redacted_referer,
        url = %redacted_request_url,
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
        "final_url={}, location={}, content_type={}, body_preview={}",
        redact_gemini_canvas_url_for_logs(&final_url),
        location
            .as_deref()
            .map(redact_gemini_canvas_url_for_logs)
            .unwrap_or_else(|| "<none>".to_string()),
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
