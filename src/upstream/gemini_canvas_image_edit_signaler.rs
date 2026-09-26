use super::*;
use rquest::{Client, Method};
use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;
use crate::protocol::gemini_web;
use crate::upstream::gemini_canvas_direct_http_helpers::{
    fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale,
    redact_gemini_canvas_api_key_for_logs,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    compact_sanitized_response_preview, summarize_gateway_error,
};
use crate::upstream::gemini_canvas_runtime_helpers::{
    current_unix_timestamp_i64, gemini_canvas_signaler_zx_token,
};

pub(crate) async fn open_gemini_canvas_image_edit_signaler_channel_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    locale_override: Option<&str>,
    timeout: Duration,
) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
    let base_url = payload.base_url.trim_end_matches('/');
    let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
    let storage_state = gateway_object_storage()?
        .read_json(&runtime.runtime_state_object_key)
        .await?;
    let storage_locale = gemini_canvas::harvest_image_edit_template_locale(&storage_state);
    let payload_locale = gemini_canvas::locale_from_payload(payload);
    let effective_locale = locale_override
        .map(str::to_string)
        .or(storage_locale.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(payload_locale);
    let page_body = fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
        http,
        plain_http,
        payload,
        session,
        &app_url,
        timeout,
        Some(effective_locale.as_str()),
    )
    .await?;
    let bootstrap_material = resolve_gemini_canvas_image_edit_signaler_bootstrap_material(
        payload,
        &storage_state,
        &page_body,
    )?;
    let account_id = bootstrap_material.account_id;
    let api_key_candidates = bootstrap_material.api_key_candidates;

    let mut failures = Vec::new();
    for api_key in api_key_candidates {
        let choose_server_url = format!(
            "https://signaler-pa.clients6.google.com/punctual/v1/chooseServer?key={api_key}"
        );
        let choose_server_body =
            gemini_canvas::build_image_edit_signaler_choose_server_body(&account_id);
        let choose_server_response =
            match send_gemini_canvas_signaler_request_refreshing_session_with_http(
                http,
                payload,
                session,
                Method::POST,
                &choose_server_url,
                Some("application/json+protobuf"),
                None,
                Some(choose_server_body),
                timeout
                    .min(Duration::from_secs(30))
                    .max(Duration::from_secs(15)),
                locale_override.or(storage_locale.as_deref()),
            )
            .await
            {
                Ok(body) => body,
                Err(error) => {
                    failures.push(format!(
                        "key={} chooseServer={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };
        let gsession_id =
            match gemini_canvas::parse_signaler_choose_server_response(&choose_server_response) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(format!(
                        "key={} chooseServerParse={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };

        let open_channel_url = format!(
            "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid={}&key={}&RID={}&CVER=22&zx={}&t=1",
            gsession_id,
            api_key,
            7000 + (current_unix_timestamp_i64().unsigned_abs() % 1000),
            gemini_canvas_signaler_zx_token()
        );
        let open_channel_body =
            gemini_canvas::build_image_edit_signaler_open_channel_body(&account_id);
        let open_channel_response =
            match send_gemini_canvas_signaler_request_refreshing_session_with_http(
                http,
                payload,
                session,
                Method::POST,
                &open_channel_url,
                Some("application/x-www-form-urlencoded"),
                Some("application/json+protobuf"),
                Some(open_channel_body),
                timeout
                    .min(Duration::from_secs(30))
                    .max(Duration::from_secs(15)),
                locale_override.or(storage_locale.as_deref()),
            )
            .await
            {
                Ok(body) => body,
                Err(error) => {
                    failures.push(format!(
                        "key={} openChannel={}",
                        redact_gemini_canvas_api_key_for_logs(&api_key),
                        summarize_gateway_error(&error)
                    ));
                    continue;
                }
            };
        let sid = match gemini_canvas::parse_signaler_open_channel_sid(&open_channel_response) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!(
                    "key={} openChannelParse={}",
                    redact_gemini_canvas_api_key_for_logs(&api_key),
                    summarize_gateway_error(&error)
                ));
                continue;
            }
        };

        return Ok(GeminiCanvasSignalerChannel {
            api_key,
            gsession_id,
            sid,
            next_aid: 0,
        });
    }

    Err(gemini_canvas_image_edit_signaler_all_keys_failed_error(
        failures.join(" | ").as_str(),
    ))
}

pub(crate) async fn prewarm_gemini_canvas_image_edit_signaler_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
    locale_override: Option<&str>,
    timeout: Duration,
) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
    let mut channel = open_gemini_canvas_image_edit_signaler_channel_with_http(
        http,
        plain_http,
        payload,
        runtime,
        session,
        locale_override,
        timeout,
    )
    .await?;
    let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(
        &channel,
        &gemini_canvas_signaler_zx_token(),
    );
    let body = send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
        http,
        payload,
        session,
        &poll_url,
        timeout
            .min(Duration::from_secs(60))
            .max(Duration::from_secs(20)),
        channel.next_aid,
        locale_override,
    )
    .await?;
    let _ = refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
        http,
        payload,
        session,
        &channel,
        &body,
        timeout
            .min(Duration::from_secs(20))
            .max(Duration::from_secs(10)),
        locale_override,
    )
    .await;
    update_gemini_canvas_image_edit_signaler_next_aid_from_body(&mut channel, &body);
    Ok(channel)
}

pub(crate) async fn prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
    http: &Client,
    plain_http: &Client,
    payload: &ProviderAccountPayload,
    runtime: &gemini_canvas::GeminiCanvasRuntime,
    storage_state: &Value,
    base_url: &str,
    app_url: &str,
    auth_user: &str,
    locale_hint: Option<&str>,
    edit_context: Option<&GeminiCanvasImageEditFollowupContext>,
    timeout: Duration,
) -> Result<
    (
        gemini_canvas::GeminiCanvasPureHttpSession,
        GeminiCanvasSignalerChannel,
    ),
    GatewayError,
> {
    if let Some((session, channel)) = edit_context.and_then(|context| {
        context
            .signaler_session
            .clone()
            .zip(context.signaler_channel.clone())
    }) {
        return Ok((session, channel));
    }

    let mut session = gemini_canvas::storage_state_to_pure_http_session(
        storage_state,
        app_url,
        base_url,
        auth_user,
    )?;
    let channel = open_gemini_canvas_image_edit_signaler_channel_with_http(
        http,
        plain_http,
        payload,
        runtime,
        &mut session,
        locale_hint,
        timeout,
    )
    .await?;
    Ok((session, channel))
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_locator_from_body(
    context: &mut GeminiCanvasImageEditFollowupContext,
    body: &str,
) {
    if context.signaler_response_id.is_some() {
        return;
    }

    if let Ok(locator) = gemini_canvas::extract_stream_generate_locator(body) {
        context.signaler_response_id = Some(locator.response_id);
        context.signaler_conversation_id = Some(locator.conversation_id);
    } else if let Some(response_id) = gemini_canvas::extract_stream_generate_response_id(body).ok()
    {
        context.signaler_response_id = Some(response_id);
    }
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_poll_body_preview(
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    body: &str,
) -> String {
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_locator_from_body(context, body);
    }
    compact_sanitized_response_preview(body, 240)
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_followup_state_from_body(
    context: &mut GeminiCanvasImageEditFollowupContext,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    body: &str,
) {
    record_gemini_canvas_image_edit_signaler_followup_state(context, session, channel, locale_hint);
    record_gemini_canvas_image_edit_signaler_locator_from_body(context, body);
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_followup_state(
    context: &mut GeminiCanvasImageEditFollowupContext,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
) {
    context.signaler_session = Some(session.clone());
    context.signaler_channel = Some(channel.clone());
    if context.locale_hint.is_none() {
        context.locale_hint = locale_hint.map(str::to_string);
    }
}

pub(crate) fn extract_gemini_canvas_image_edit_signaler_assets_from_body(
    body: &str,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError> {
    let assets = gemini_canvas::extract_page_blob_media_assets(
        body,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )?;
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_followup_state_from_body(
            context,
            session,
            channel,
            locale_hint,
            body,
        );
    }
    Ok(assets)
}

pub(crate) fn try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
    body: String,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> Option<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String)> {
    let assets = extract_gemini_canvas_image_edit_signaler_assets_from_body(
        &body,
        session,
        channel,
        locale_hint,
        edit_context,
    )
    .ok()?;
    Some((assets, body))
}

pub(crate) fn finish_gemini_canvas_image_edit_signaler_missing_asset(
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
    channel: &GeminiCanvasSignalerChannel,
    locale_hint: Option<&str>,
    failures: &[String],
    last_body_preview: Option<String>,
) -> GatewayError {
    if let Some(context) = edit_context {
        record_gemini_canvas_image_edit_signaler_followup_state(
            context,
            session,
            channel,
            locale_hint,
        );
    }

    let failure_summary = failures.join(" | ");
    let last_body_preview_text = last_body_preview.unwrap_or_else(|| "<none>".to_string());
    gemini_canvas_image_edit_signaler_missing_asset_error(
        channel.next_aid,
        &failure_summary,
        &last_body_preview_text,
    )
}

pub(crate) fn record_gemini_canvas_image_edit_signaler_app_path(
    base_url: &str,
    app_path: &str,
    seen_app_paths: &mut HashSet<String>,
    first_app_path_seen_at: &mut Option<Instant>,
    edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
) -> String {
    let page_url = format!("{base_url}{app_path}");
    append_gemini_canvas_image_edit_trace("signaler.app-path", || page_url.as_str());
    seen_app_paths.insert(page_url.clone());
    if first_app_path_seen_at.is_none() {
        *first_app_path_seen_at = Some(Instant::now());
    }
    if let Some(context) = edit_context {
        if !context
            .signaler_app_urls
            .iter()
            .any(|value| value == &page_url)
        {
            context.signaler_app_urls.push(page_url.clone());
        }
        context.signaler_app_url = Some(page_url.clone());
    }
    page_url
}

pub(crate) fn build_gemini_canvas_image_edit_signaler_poll_url(
    channel: &GeminiCanvasSignalerChannel,
    zx_token: &str,
) -> String {
    format!(
        "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid={}&key={}&RID=rpc&SID={}&AID={}&CI=0&TYPE=xmlhttp&zx={}&t=1",
        channel.gsession_id,
        channel.api_key,
        channel.sid,
        channel.next_aid,
        zx_token
    )
}

pub(crate) fn update_gemini_canvas_image_edit_signaler_next_aid_from_body(
    channel: &mut GeminiCanvasSignalerChannel,
    body: &str,
) -> Option<u64> {
    let max_aid = gemini_canvas::extract_signaler_long_poll_max_aid(body)?;
    channel.next_aid = max_aid;
    Some(max_aid)
}

pub(crate) fn gemini_canvas_image_edit_signaler_page_failure_entry(
    page_url: &str,
    label: &str,
    detail: impl AsRef<str>,
) -> String {
    format!("page_url={} {}={}", page_url, label, detail.as_ref())
}

pub(crate) fn gemini_canvas_image_edit_signaler_poll_error_entry(
    next_aid: u64,
    detail: impl AsRef<str>,
) -> String {
    format!("poll_aid={} error={}", next_aid, detail.as_ref())
}

pub(crate) fn gemini_canvas_image_edit_signaler_refresh_error_entry(
    next_aid: u64,
    detail: impl AsRef<str>,
) -> String {
    format!("refresh_creds aid={} error={}", next_aid, detail.as_ref())
}

#[path = "gemini_canvas_image_edit_signaler_transport.rs"]
mod transport;
pub(crate) use transport::*;
