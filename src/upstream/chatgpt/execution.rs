use std::collections::HashMap;
use std::time::Duration;

use rquest::Client;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::chatgpt::web_reverse as surface;
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::response_types::UpstreamStreamingResponse;

use super::bootstrap::bootstrap_site;
use super::common::{classify_chatgpt_web_text_response, is_event_stream_content_type};
use super::web_reverse::{build_request_context, translate_chatgpt_web_stream};
use super::PROVIDER;

mod policy;
mod requirements;
mod transport;

use policy::{
    cached_f_conversation_requirements, cached_f_conversation_turn_trace_id, conversation_path,
    conversation_prepare_path, extra_body_string, should_post_prepare,
};
use requirements::get_requirements;
use transport::{post_json, post_prepare};

pub async fn execute(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let request_context = build_request_context(payload);
    let cached_requirements = cached_f_conversation_requirements(payload);
    let using_cached_f_conversation_material = cached_requirements.is_some();
    let requirements = if let Some(requirements) = cached_requirements {
        requirements
    } else {
        let bootstrap =
            bootstrap_site(http, timeout, payload, &request_context, extra_headers).await?;
        get_requirements(
            http,
            timeout,
            payload,
            &request_context,
            &bootstrap,
            extra_headers,
        )
        .await?
    };
    let timezone = extra_body_string(payload, &["timezone"])
        .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string());
    let body = surface::pack_request(req, model, &timezone)?;
    let turn_trace_id = cached_f_conversation_turn_trace_id(payload)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conversation_path = conversation_path(payload, using_cached_f_conversation_material);
    if should_post_prepare(payload, using_cached_f_conversation_material) {
        if let Some(prepare_path) = conversation_prepare_path(payload, &conversation_path) {
            post_prepare(
                http,
                timeout,
                payload,
                &request_context,
                extra_headers,
                &prepare_path,
                &turn_trace_id,
            )
            .await?;
        }
    }
    let response = post_json(
        http,
        timeout,
        payload,
        &request_context,
        extra_headers,
        &conversation_path,
        &body,
        Some(&requirements),
        true,
        Some(&turn_trace_id),
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = collect_bounded_upstream_charset_text_with_provider(
        response,
        "ChatGPT Web reverse conversation body",
        PROVIDER,
    )
    .await?;
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;

    surface::accumulate_response(&body_text, model).map_err(|error| error.with_provider(PROVIDER))
}

pub async fn execute_stream(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let request_context = build_request_context(payload);
    let cached_requirements = cached_f_conversation_requirements(payload);
    let using_cached_f_conversation_material = cached_requirements.is_some();
    let requirements = if let Some(requirements) = cached_requirements {
        requirements
    } else {
        let bootstrap =
            bootstrap_site(http, timeout, payload, &request_context, extra_headers).await?;
        get_requirements(
            http,
            timeout,
            payload,
            &request_context,
            &bootstrap,
            extra_headers,
        )
        .await?
    };
    let timezone = extra_body_string(payload, &["timezone"])
        .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string());
    let body = surface::pack_request(req, model, &timezone)?;
    let turn_trace_id = cached_f_conversation_turn_trace_id(payload)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conversation_path = conversation_path(payload, using_cached_f_conversation_material);
    if should_post_prepare(payload, using_cached_f_conversation_material) {
        if let Some(prepare_path) = conversation_prepare_path(payload, &conversation_path) {
            post_prepare(
                http,
                timeout,
                payload,
                &request_context,
                extra_headers,
                &prepare_path,
                &turn_trace_id,
            )
            .await?;
        }
    }
    let response = post_json(
        http,
        timeout,
        payload,
        &request_context,
        extra_headers,
        &conversation_path,
        &body,
        Some(&requirements),
        true,
        Some(&turn_trace_id),
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !(200..300).contains(&status) {
        let body_text = collect_bounded_upstream_charset_text_with_provider(
            response,
            "ChatGPT Web reverse conversation body",
            PROVIDER,
        )
        .await?;
        classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;
        unreachable!("non-success ChatGPT web reverse response unexpectedly passed validation");
    }
    if !is_event_stream_content_type(content_type.as_deref()) {
        let body_text = collect_bounded_upstream_charset_text_with_provider(
            response,
            "ChatGPT Web reverse conversation body",
            PROVIDER,
        )
        .await?;
        classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;
        return Err(GatewayError::server_error(
            "ChatGPT Web reverse streaming response did not use the text/event-stream content type.",
        )
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_non_sse_response"));
    }
    Ok(UpstreamStreamingResponse::Bytes(Box::pin(
        translate_chatgpt_web_stream(response.bytes_stream(), model.to_string()),
    )))
}

#[cfg(test)]
mod tests;
